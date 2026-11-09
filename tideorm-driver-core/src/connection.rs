use crate::database::{Database, HasStatementCache};
use crate::error::Error;

use crate::sql_str::SqlSafeStr;
use crate::transaction::{Transaction, TransactionManager};
use futures_core::future::BoxFuture;
use percent_encoding::percent_decode_str;
use std::borrow::Cow;
use std::fmt::Debug;
use std::future::Future;
use std::str::FromStr;
use url::Url;

/// Represents a single database connection.
pub trait Connection: Send {
    type Database: Database<Connection = Self>;

    type Options: ConnectOptions<Connection = Self>;

    /// Explicitly close this database connection.
    ///
    /// This notifies the database server that the connection is closing so that it can
    /// free up any server-side resources in use.
    ///
    /// While connections can simply be dropped to clean up local resources,
    /// the `Drop` handler itself cannot notify the server that the connection is being closed
    /// because that may require I/O to send a termination message. That can result in a delay
    /// before the server learns that the connection is gone, usually from a TCP keepalive timeout.
    ///
    /// Creating and dropping many connections in short order without calling `.close()` may
    /// lead to errors from the database server because those senescent connections will still
    /// count against any connection limit or quota that is configured.
    ///
    /// Therefore it is recommended to call `.close()` on a connection when you are done using it
    /// and to `.await` the result to ensure the termination message is sent.
    fn close(self) -> impl Future<Output = Result<(), Error>> + Send + 'static;

    /// Immediately close the connection without sending a graceful shutdown.
    ///
    /// This should still at least send a TCP `FIN` frame to let the server know we're dying.
    #[doc(hidden)]
    fn close_hard(self) -> impl Future<Output = Result<(), Error>> + Send + 'static;

    /// Checks if a connection to the database is still valid.
    fn ping(&mut self) -> impl Future<Output = Result<(), Error>> + Send + '_;

    /// Begin a new transaction or establish a savepoint within the active transaction.
    ///
    /// Returns a [`Transaction`] for controlling and tracking the new transaction.
    fn begin(
        &mut self,
    ) -> impl Future<Output = Result<Transaction<'_, Self::Database>, Error>> + Send + '_
    where
        Self: Sized,
    {
        Transaction::begin(self, None)
    }

    /// Begin a new transaction with a custom statement.
    ///
    /// Returns a [`Transaction`] for controlling and tracking the new transaction.
    ///
    /// Returns an error if the connection is already in a transaction or if
    /// `statement` does not put the connection into a transaction.
    fn begin_with(
        &mut self,
        statement: impl SqlSafeStr,
    ) -> impl Future<Output = Result<Transaction<'_, Self::Database>, Error>> + Send + '_
    where
        Self: Sized,
    {
        Transaction::begin(self, Some(statement.into_sql_str()))
    }

    /// Returns `true` if the connection is currently in a transaction.
    ///
    /// # Note: Automatic Rollbacks May Not Be Counted
    /// Certain database errors (such as a serializable isolation failure)
    /// can cause automatic rollbacks of a transaction
    /// which may not be indicated in the return value of this method.
    #[inline]
    fn is_in_transaction(&self) -> bool {
        <Self::Database as Database>::TransactionManager::get_transaction_depth(self) != 0
    }

    /// Returns the server's last reported transaction state, independently of
    /// the driver's nesting depth. Pending replies may not have been read yet.
    fn transaction_state(&self) -> crate::transaction::TransactionState {
        <Self::Database as Database>::TransactionManager::get_transaction_state(self)
    }

    /// Execute the function inside a transaction.
    ///
    /// If the function returns an error, the transaction will be rolled back. If it does not
    /// return an error, the transaction will be committed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use driver::postgres::{PgConnection, PgRow};
    /// use driver::Connection;
    ///
    /// # pub async fn _f(conn: &mut PgConnection) -> driver::Result<Vec<PgRow>> {
    /// conn.transaction(|txn| Box::pin(async move {
    ///     driver::query("select * from ..").fetch_all(&mut **txn).await
    /// })).await
    /// # }
    /// ```
    fn transaction<'a, F, R, E>(
        &'a mut self,
        callback: F,
    ) -> impl Future<Output = Result<R, E>> + Send + 'a
    where
        for<'c> F: FnOnce(&'c mut Transaction<'_, Self::Database>) -> BoxFuture<'c, Result<R, E>>
            + 'a
            + Send
            + Sync,
        Self: Sized,
        R: Send,
        E: From<Error> + Send,
    {
        async move {
            let mut transaction = self.begin().await?;
            let ret = callback(&mut transaction).await;

            match ret {
                Ok(ret) => {
                    transaction.commit().await?;

                    Ok(ret)
                }
                Err(err) => {
                    transaction.rollback().await?;

                    Err(err)
                }
            }
        }
    }

    /// The number of statements currently cached in the connection.
    fn cached_statements_size(&self) -> usize
    where
        Self::Database: HasStatementCache,
    {
        0
    }

    /// Removes all statements from the cache, closing them on the server if
    /// needed.
    fn clear_cached_statements(&mut self) -> impl Future<Output = Result<(), Error>> + Send + '_
    where
        Self::Database: HasStatementCache,
    {
        async move { Ok(()) }
    }

    /// Restore any buffers in the connection to their default capacity, if possible.
    ///
    /// Sending a large query or receiving a resultset with many columns can cause the connection
    /// to allocate additional buffer space to fit the data which is retained afterwards in
    /// case it's needed again. This can give the outward appearance of a memory leak, but is
    /// in fact the intended behavior.
    ///
    /// Calling this method tells the connection to release that excess memory if it can,
    /// though be aware that calling this too often can cause unnecessary thrashing or
    /// fragmentation in the global allocator. If there's still data in the connection buffers
    /// (unlikely if the last query was run to completion) then it may need to be moved to
    /// allow the buffers to shrink.
    fn shrink_buffers(&mut self);

    #[doc(hidden)]
    fn flush(&mut self) -> impl Future<Output = Result<(), Error>> + Send + '_;

    /// Establish a new database connection.
    ///
    /// A value of [`Options`][Self::Options] is parsed from the provided connection string. This parsing
    /// is database-specific.
    #[inline]
    fn connect(url: &str) -> impl Future<Output = Result<Self, Error>> + Send + 'static
    where
        Self: Sized,
    {
        let options = url.parse();

        async move { Self::connect_with(&options?).await }
    }

    /// Establish a new database connection with the provided options.
    fn connect_with(
        options: &Self::Options,
    ) -> impl Future<Output = Result<Self, Error>> + Send + '_
    where
        Self: Sized,
    {
        options.connect()
    }
}

pub trait ConnectOptions: 'static + Send + Sync + FromStr<Err = Error> + Debug + Clone {
    type Connection: Connection<Options = Self> + ?Sized;

    /// Parse the `ConnectOptions` from a URL.
    fn from_url(url: &Url) -> Result<Self, Error>;

    /// Establish a new database connection with the options specified by `self`.
    fn connect(&self) -> impl Future<Output = Result<Self::Connection, Error>> + Send + '_
    where
        Self::Connection: Sized;
}

/// Percent-decodes a part of a connection URL (user name, password, database or file name)
/// as UTF-8.
pub fn decode_url_part(part: &str) -> Result<Cow<'_, str>, Error> {
    percent_decode_str(part)
        .decode_utf8()
        .map_err(Error::config)
}

#[cfg(test)]
#[path = "../tests/unit/connection_tests.rs"]
mod tests;
