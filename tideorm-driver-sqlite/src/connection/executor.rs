use crate::{
    Sqlite, SqliteConnection, SqliteQueryResult, SqliteRow, SqliteStatement, SqliteTypeInfo,
};
use driver_core::error::Error;
use driver_core::executor::{Execute, Executor};
use driver_core::sql_str::SqlStr;
use driver_core::Either;
use futures_core::Stream;
use futures_util::{TryFutureExt, TryStreamExt};
use std::{future::Future, pin::pin};

impl<'c> Executor<'c> for &'c mut SqliteConnection {
    type Database = Sqlite;

    fn fetch_many<'e, 'q, E>(
        self,
        mut query: E,
    ) -> impl Stream<Item = Result<Either<SqliteQueryResult, SqliteRow>, Error>> + Send + 'e
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
        'q: 'e,
        E: 'q,
    {
        let arguments = query.take_arguments().map_err(Error::Encode);
        let persistent = query.persistent();
        let sql = query.sql();

        async move {
            let arguments = arguments?;
            let persistent = persistent && arguments.is_some();
            self.worker
                .execute(sql, arguments, self.row_channel_size, persistent, None)
                .await
        }
        .map_ok(flume::Receiver::into_stream)
        .try_flatten_stream()
    }

    fn fetch_optional<'e, 'q, E>(
        self,
        mut query: E,
    ) -> impl Future<Output = Result<Option<SqliteRow>, Error>> + Send + 'e
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
        'q: 'e,
        E: 'q,
    {
        let arguments = query.take_arguments().map_err(Error::Encode);
        let persistent = query.persistent();

        async move {
            let arguments = arguments?;
            let persistent = persistent && arguments.is_some();
            let sql = query.sql();
            let mut stream = pin!(self
                .worker
                .execute(sql, arguments, self.row_channel_size, persistent, Some(1))
                .map_ok(flume::Receiver::into_stream)
                .try_flatten_stream());

            while let Some(res) = stream.try_next().await? {
                if let Either::Right(row) = res {
                    return Ok(Some(row));
                }
            }

            Ok(None)
        }
    }

    async fn prepare_with<'e>(
        self,
        sql: SqlStr,
        _parameters: &'e [SqliteTypeInfo],
    ) -> Result<SqliteStatement, Error>
    where
        'c: 'e,
    {
        self.worker.prepare(sql).await
    }
}
