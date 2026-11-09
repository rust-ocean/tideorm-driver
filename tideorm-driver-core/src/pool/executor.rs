use std::future::Future;
use std::pin::pin;

use either::Either;
use futures_core::stream::Stream;
use futures_util::TryStreamExt;

use crate::database::Database;
use crate::error::Error;
use crate::executor::{Execute, Executor};
use crate::pool::Pool;
use crate::sql_str::SqlStr;

impl<'p, DB: Database> Executor<'p> for &'_ Pool<DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
{
    type Database = DB;

    fn fetch_many<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Stream<Item = Result<Either<DB::QueryResult, DB::Row>, Error>> + Send + 'e
    where
        E: 'q + Execute<'q, Self::Database>,
    {
        let pool = self.clone();

        try_stream! {
            let mut conn = pool.acquire().await?;
            let mut s = pin!(conn.fetch_many(query));

            while let Some(v) = s.try_next().await? {
                r#yield!(v);
            }

            Ok(())
        }
    }

    fn fetch_optional<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> impl Future<Output = Result<Option<DB::Row>, Error>> + Send + 'e
    where
        E: 'q + Execute<'q, Self::Database>,
    {
        let pool = self.clone();

        async move { pool.acquire().await?.fetch_optional(query).await }
    }

    fn prepare_with<'e>(
        self,
        sql: SqlStr,
        parameters: &'e [<Self::Database as Database>::TypeInfo],
    ) -> impl Future<Output = Result<<Self::Database as Database>::Statement, Error>> + Send + 'e
    where
        'p: 'e,
    {
        let pool = self.clone();

        async move { pool.acquire().await?.prepare_with(sql, parameters).await }
    }
}
