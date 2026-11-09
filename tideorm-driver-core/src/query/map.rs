//! `Map`, a query whose rows are mapped to another type as they arrive.

use std::future;
use std::pin::pin;

use futures_core::stream::Stream;
use futures_util::{TryFutureExt, TryStreamExt};

use super::Query;
use crate::arguments::IntoArguments;
use crate::database::Database;
use crate::error::{BoxDynError, Error};
use crate::executor::{Execute, Executor};
use crate::sql_str::SqlStr;

/// A single SQL query that will map its results to an owned Rust type.
///
/// Executes as a prepared statement.
///
/// Returned by [`Query::try_map`]. Has most of the same methods as [`Query`] but the return
/// types are changed to reflect the mapping. However, there is no equivalent of [`Query::execute`]
/// as it doesn't make sense to map the result type and then ignore it.
///
/// [`Query::bind`] is also omitted; place `.bind()` calls before `.try_map()` to prevent adding
/// superfluous binds to the mapped query.
#[must_use = "query must be executed to affect database"]
pub struct Map<'q, DB: Database, F, A> {
    pub(super) inner: Query<'q, DB, A>,
    pub(super) mapper: F,
}

impl<'q, DB, F: Send, A: Send> Execute<'q, DB> for Map<'q, DB, F, A>
where
    DB: Database,
    A: IntoArguments<DB>,
{
    #[inline]
    fn sql(self) -> SqlStr {
        self.inner.sql()
    }

    #[inline]
    fn statement(&self) -> Option<&DB::Statement> {
        self.inner.statement()
    }

    #[inline]
    fn take_arguments(&mut self) -> Result<Option<<DB as Database>::Arguments>, BoxDynError> {
        self.inner.take_arguments()
    }

    #[inline]
    fn persistent(&self) -> bool {
        self.inner.arguments.is_some()
    }
}

impl<'q, DB, F, O, A> Map<'q, DB, F, A>
where
    DB: Database,
    F: FnMut(DB::Row) -> Result<O, Error> + Send,
    O: Send + Unpin,
    A: 'q + Send + IntoArguments<DB>,
{
    /// Map each row in the result to another type.
    ///
    /// See [`try_map`](Map::try_map) for a fallible version of this method.
    #[inline]
    pub fn map<G, P>(
        self,
        mut g: G,
    ) -> Map<'q, DB, impl FnMut(DB::Row) -> Result<P, Error> + Send, A>
    where
        G: FnMut(O) -> P + Send,
        P: Unpin,
    {
        self.try_map(move |data| Ok(g(data)))
    }

    /// Map each row in the result to another type.
    #[inline]
    pub fn try_map<G, P>(
        self,
        mut g: G,
    ) -> Map<'q, DB, impl FnMut(DB::Row) -> Result<P, Error> + Send, A>
    where
        G: FnMut(O) -> Result<P, Error> + Send,
        P: Unpin,
    {
        let mut f = self.mapper;
        Map {
            inner: self.inner,
            mapper: move |row| f(row).and_then(&mut g),
        }
    }

    /// Execute the query and return the generated results as a stream.
    pub fn fetch<'e, 'c: 'e, E>(
        mut self,
        executor: E,
    ) -> impl Stream<Item = Result<O, Error>> + Send + 'e
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        F: 'e,
        O: 'e,
    {
        try_stream! {
            let mut s = pin!(executor.fetch(self.inner));

            while let Some(row) = s.try_next().await? {
                r#yield!((self.mapper)(row)?);
            }

            Ok(())
        }
    }

    /// Execute the query and return all the resulting rows collected into a [`Vec`].
    ///
    /// ### Note: beware result set size.
    /// This will attempt to collect the full result set of the query into memory.
    ///
    /// To avoid exhausting available memory, ensure the result set has a known upper bound,
    /// e.g. using `LIMIT`.
    pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        F: 'e,
        O: 'e,
    {
        self.fetch(executor).try_collect().await
    }

    /// Execute the query, returning the first row or [`Error::RowNotFound`] otherwise.
    ///
    /// ### Note: for best performance, ensure the query returns at most one row.
    /// Depending on the driver implementation, if your query can return more than one row,
    /// it may lead to wasted CPU time and bandwidth on the database server.
    ///
    /// Even when the driver implementation takes this into account, ensuring the query returns at most one row
    /// can result in a more optimal query plan.
    ///
    /// If your query has a `WHERE` clause filtering a unique column by a single value, you're good.
    ///
    /// Otherwise, you might want to add `LIMIT 1` to your query.
    pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<O, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        F: 'e,
        O: 'e,
    {
        self.fetch_optional(executor)
            .and_then(|row| {
                future::ready(match row {
                    Some(row) => Ok(row),
                    None => Err(Error::RowNotFound),
                })
            })
            .await
    }

    /// Execute the query, returning the first row or `None` otherwise.
    ///
    /// ### Note: for best performance, ensure the query returns at most one row.
    /// Depending on the driver implementation, if your query can return more than one row,
    /// it may lead to wasted CPU time and bandwidth on the database server.
    ///
    /// Even when the driver implementation takes this into account, ensuring the query returns at most one row
    /// can result in a more optimal query plan.
    ///
    /// If your query has a `WHERE` clause filtering a unique column by a single value, you're good.
    ///
    /// Otherwise, you might want to add `LIMIT 1` to your query.
    pub async fn fetch_optional<'e, 'c: 'e, E>(mut self, executor: E) -> Result<Option<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        F: 'e,
        O: 'e,
    {
        let row = executor.fetch_optional(self.inner).await?;

        if let Some(row) = row {
            (self.mapper)(row).map(Some)
        } else {
            Ok(None)
        }
    }
}
