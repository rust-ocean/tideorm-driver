use crate::database::Database;
use crate::error::{BoxDynError, Error};
use crate::sql_str::{SqlSafeStr, SqlStr};

use either::Either;
use futures_core::stream::Stream;
use futures_util::{TryFutureExt, TryStreamExt};
use std::fmt::Debug;
use std::future::{self, Future};

/// A type that contains or can provide a database
/// connection to use for executing queries against the database.
///
/// No guarantees are provided that successive queries run on the same
/// physical database connection.
///
/// A [`Connection`](crate::connection::Connection) is an `Executor` that guarantees that
/// successive queries are ran on the same physical database connection.
///
/// Implemented for the following:
///
///  * [`&Pool`](super::pool::Pool)
///  * [`&mut Connection`](super::connection::Connection)
///
/// The [`Executor`] impls for [`Transaction`](crate::transaction::Transaction)
/// and [`PoolConnection`](crate::pool::PoolConnection) have been deleted because they
/// cannot exist in the new crate architecture without rewriting the Executor trait entirely.
/// To fix this breakage, simply add a dereference where an impl [`Executor`] is expected, as
/// they both dereference to the inner connection type which will still implement it:
/// * `&mut transaction` -> `&mut *transaction`
/// * `&mut connection` -> `&mut *connection`
///
/// # Note: Methods Not for General Use
/// This is a building-block trait that is exposed mainly for use as a trait bound.
///
/// Instead of calling the methods of this trait, use the free functions in the `driver` crate root:
/// * [`driver::query()`]: use for DML queries (`SELECT`, `INSERT`, `UPDATE`, `DELETE`, etc.)
///     * uses and caches prepared statements internally per-connection
///     * may use argument placeholders (MySQL/MariaDB: `?`, Postgres/SQLite: `$1`, `$2`, etc.)
///     * one query per string
///
/// [`driver::query()`]: crate::query::query
pub trait Executor<'c>: Send + Debug + Sized {
    type Database: Database;

    /// Execute the query and return the total number of rows affected.
    fn execute<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Future<Output = Result<<Self::Database as Database>::QueryResult, Error>> + Send + 'e
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>,
    {
        self.execute_many(query).try_collect()
    }

    /// Execute multiple queries and return the rows affected from each query, in a stream.
    fn execute_many<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Stream<Item = Result<<Self::Database as Database>::QueryResult, Error>> + Send + 'e
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>,
    {
        self.fetch_many(query)
            .try_filter_map(|step| future::ready(Ok(step.left())))
    }

    /// Execute the query and return the generated results as a stream.
    fn fetch<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Stream<Item = Result<<Self::Database as Database>::Row, Error>> + Send + 'e
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>,
    {
        self.fetch_many(query)
            .try_filter_map(|step| future::ready(Ok(step.right())))
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    fn fetch_many<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Stream<
        Item = Result<
            Either<<Self::Database as Database>::QueryResult, <Self::Database as Database>::Row>,
            Error,
        >,
    > + Send
           + 'e
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>;

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    fn fetch_all<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Future<Output = Result<Vec<<Self::Database as Database>::Row>, Error>> + Send + 'e
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>,
    {
        self.fetch(query).try_collect()
    }

    /// Execute the query and returns exactly one row.
    fn fetch_one<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Future<Output = Result<<Self::Database as Database>::Row, Error>> + Send + 'e
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>,
    {
        self.fetch_optional(query)
            .and_then(|row| future::ready(row.ok_or(Error::RowNotFound)))
    }

    /// Execute the query and returns at most one row.
    fn fetch_optional<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Future<Output = Result<Option<<Self::Database as Database>::Row>, Error>> + Send + 'e
    where
        'c: 'e,
        E: 'q + Execute<'q, Self::Database>;

    /// Prepare the SQL query to inspect the type information of its parameters
    /// and results.
    ///
    /// Be advised that when using the `query` function, the query is transparently prepared and
    /// executed.
    ///
    /// This explicit API is provided to allow access to the statement metadata available after
    /// it prepared but before the first row is returned.
    #[inline]
    fn prepare<'e>(
        self,
        query: SqlStr,
    ) -> impl Future<Output = Result<<Self::Database as Database>::Statement, Error>> + Send + 'e
    where
        'c: 'e,
    {
        self.prepare_with(query, &[])
    }

    /// Prepare the SQL query, with parameter type information, to inspect the
    /// type information about its parameters and results.
    ///
    /// PostgreSQL can take advantage of this extra information to influence
    /// parameter type inference.
    fn prepare_with<'e>(
        self,
        sql: SqlStr,
        parameters: &'e [<Self::Database as Database>::TypeInfo],
    ) -> impl Future<Output = Result<<Self::Database as Database>::Statement, Error>> + Send + 'e
    where
        'c: 'e;
}

/// A type that may be executed against a database connection.
///
/// Implemented for the following:
///
///  * [`&str`](std::str)
///  * [`Query`](super::query::Query)
///
pub trait Execute<'q, DB: Database>: Send + Sized {
    /// Gets the SQL that will be executed.
    fn sql(self) -> SqlStr;

    /// Gets the previously cached statement, if available.
    fn statement(&self) -> Option<&DB::Statement>;

    /// Returns the arguments to be bound against the query string.
    ///
    /// Returning `Ok(None)` for `Arguments` indicates to use a "simple" query protocol and to not
    /// prepare the query. Returning `Ok(Some(Default::default()))` is an empty arguments object that
    /// will be prepared (and cached) before execution.
    ///
    /// Returns `Err` if encoding any of the arguments failed.
    fn take_arguments(&mut self) -> Result<Option<<DB as Database>::Arguments>, BoxDynError>;

    /// Returns `true` if the statement should be cached.
    fn persistent(&self) -> bool;
}

impl<DB: Database, T> Execute<'_, DB> for T
where
    T: SqlSafeStr + Send,
{
    #[inline]
    fn sql(self) -> SqlStr {
        self.into_sql_str()
    }

    #[inline]
    fn statement(&self) -> Option<&DB::Statement> {
        None
    }

    #[inline]
    fn take_arguments(&mut self) -> Result<Option<<DB as Database>::Arguments>, BoxDynError> {
        Ok(None)
    }

    #[inline]
    fn persistent(&self) -> bool {
        true
    }
}

impl<DB: Database, T> Execute<'_, DB> for (T, Option<<DB as Database>::Arguments>)
where
    T: SqlSafeStr + Send,
{
    #[inline]
    fn sql(self) -> SqlStr {
        self.0.into_sql_str()
    }

    #[inline]
    fn statement(&self) -> Option<&DB::Statement> {
        None
    }

    #[inline]
    fn take_arguments(&mut self) -> Result<Option<<DB as Database>::Arguments>, BoxDynError> {
        Ok(self.1.take())
    }

    #[inline]
    fn persistent(&self) -> bool {
        true
    }
}
