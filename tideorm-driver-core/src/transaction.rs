use std::fmt::{self, Debug, Formatter};
use std::future::{self, Future};
use std::ops::{Deref, DerefMut};

use futures_core::future::BoxFuture;

use crate::database::Database;
use crate::error::Error;
use crate::pool::MaybePoolConnection;
use crate::sql_str::{AssertSqlSafe, SqlSafeStr, SqlStr};

/// The transaction state most recently reported by the database server.
///
/// Pending replies must be flushed before using this to decide whether a
/// connection can be reused. This is independent of the driver's nesting depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// The server is outside a transaction.
    Idle,
    /// The server is inside a transaction.
    Active,
    /// The server is inside a failed transaction that must be rolled back.
    Failed,
    /// A failure abandoned the whole transaction the driver still counts
    /// open: SQLite rolled it back by itself, or a statement deadlocked.
    /// Nothing more runs in it; it must be rolled back.
    Aborted,
}

/// Generic management of database transactions.
///
/// This trait should not be used, except when implementing [`Connection`](crate::connection::Connection).
pub trait TransactionManager {
    type Database: Database;

    /// Begin a new transaction or establish a savepoint within the active transaction.
    ///
    /// If this is a new transaction, `statement` may be used instead of the
    /// default "BEGIN" statement.
    ///
    /// If we are already inside a transaction and `statement.is_some()`, then
    /// `Error::InvalidSavePoint` is returned without running any statements.
    fn begin(
        conn: &mut <Self::Database as Database>::Connection,
        statement: Option<SqlStr>,
    ) -> impl Future<Output = Result<(), Error>> + Send + '_;

    /// Commit the active transaction or release the most recent savepoint.
    fn commit(
        conn: &mut <Self::Database as Database>::Connection,
    ) -> impl Future<Output = Result<(), Error>> + Send + '_;

    /// Abort the active transaction or restore from the most recent savepoint.
    /// With depth zero, also abort an unowned transaction reported by the server.
    fn rollback(
        conn: &mut <Self::Database as Database>::Connection,
    ) -> impl Future<Output = Result<(), Error>> + Send + '_;

    /// Starts to abort the active transaction or restore from the most recent snapshot.
    fn start_rollback(conn: &mut <Self::Database as Database>::Connection);

    /// Returns the server's last reported transaction state.
    fn get_transaction_state(conn: &<Self::Database as Database>::Connection) -> TransactionState;

    /// Returns the current transaction depth.
    ///
    /// Transaction depth indicates the level of nested transactions:
    /// - Level 0: No active transaction.
    /// - Level 1: A transaction is active.
    /// - Level 2 or higher: A transaction is active and one or more SAVEPOINTs have been created within it.
    fn get_transaction_depth(conn: &<Self::Database as Database>::Connection) -> usize;
}

/// An in-progress database transaction or savepoint.
///
/// A transaction starts with a call to [`Pool::begin`] or [`Connection::begin`].
///
/// A transaction should end with a call to [`commit`] or [`rollback`]. If neither are called
/// before the transaction goes out-of-scope, [`rollback`] is called. In other
/// words, [`rollback`] is called on `drop` if the transaction is still in-progress.
///
/// A savepoint is a special mark inside a transaction that allows all commands that are
/// executed after it was established to be rolled back, restoring the transaction state to
/// what it was at the time of the savepoint.
///
/// A transaction can be used as an [`Executor`] when performing queries:
/// ```rust,no_run
/// # use driver::Acquire;
/// # async fn example() -> driver::Result<()> {
/// # let id = 1;
/// # let mut conn: driver::PgConnection = unimplemented!();
/// let mut tx = conn.begin().await?;
///
/// let result = driver::query("DELETE FROM \"testcases\" WHERE id = $1")
///     .bind(id)
///     .execute(&mut *tx)
///     .await?
///     .rows_affected();
///
/// tx.commit().await
/// # }
/// ```
/// [`Executor`]: crate::executor::Executor
/// [`Connection::begin`]: crate::connection::Connection::begin()
/// [`Pool::begin`]: crate::pool::Pool::begin()
/// [`commit`]: Self::commit()
/// [`rollback`]: Self::rollback()
pub struct Transaction<'c, DB>
where
    DB: Database,
{
    connection: MaybePoolConnection<'c, DB>,
    open: bool,
}

impl<'c, DB> Transaction<'c, DB>
where
    DB: Database,
{
    #[doc(hidden)]
    pub fn begin(
        conn: impl Into<MaybePoolConnection<'c, DB>>,
        statement: Option<SqlStr>,
    ) -> BoxFuture<'c, Result<Self, Error>> {
        let conn = conn.into();

        Box::pin(async move {
            let mut tx = Self {
                connection: conn,

                // If the call to `begin` fails or doesn't complete we want to attempt a rollback in case the transaction was started.
                open: true,
            };

            DB::TransactionManager::begin(&mut tx.connection, statement).await?;

            Ok(tx)
        })
    }

    /// Commits this transaction or savepoint.
    pub async fn commit(mut self) -> Result<(), Error> {
        DB::TransactionManager::commit(&mut self.connection).await?;
        self.open = false;

        Ok(())
    }

    /// Aborts this transaction or savepoint.
    pub async fn rollback(mut self) -> Result<(), Error> {
        DB::TransactionManager::rollback(&mut self.connection).await?;
        self.open = false;

        Ok(())
    }
}

impl<DB> Debug for Transaction<'_, DB>
where
    DB: Database,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transaction").finish()
    }
}

impl<DB> Deref for Transaction<'_, DB>
where
    DB: Database,
{
    type Target = DB::Connection;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl<DB> DerefMut for Transaction<'_, DB>
where
    DB: Database,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

// Implement `AsMut<DB::Connection>` so `Transaction` can be given to a
// `PgAdvisoryLockGuard`.
impl<DB: Database> AsMut<DB::Connection> for Transaction<'_, DB> {
    fn as_mut(&mut self) -> &mut DB::Connection {
        &mut self.connection
    }
}

impl<'t, DB: Database> crate::acquire::Acquire<'t> for &'t mut Transaction<'_, DB> {
    type Database = DB;

    type Connection = &'t mut <DB as Database>::Connection;

    #[inline]
    fn acquire(self) -> BoxFuture<'t, Result<Self::Connection, Error>> {
        Box::pin(future::ready(Ok(&mut **self)))
    }

    #[inline]
    fn begin(self) -> BoxFuture<'t, Result<Transaction<'t, DB>, Error>> {
        Transaction::begin(&mut **self, None)
    }
}

impl<DB> Drop for Transaction<'_, DB>
where
    DB: Database,
{
    fn drop(&mut self) {
        if self.open {
            // starts a rollback operation

            // what this does depends on the database but generally this means we queue a rollback
            // operation that will happen on the next asynchronous invocation of the underlying
            // connection (including if the connection is returned to a pool)

            DB::TransactionManager::start_rollback(&mut self.connection);
        }
    }
}

/// The statement that opens the level at `depth`: the caller's own, which may only
/// begin a transaction (a nested level must be a savepoint), or the default.
pub fn begin_statement(depth: usize, statement: Option<SqlStr>) -> Result<SqlStr, Error> {
    match statement {
        Some(_) if depth > 0 => Err(Error::InvalidSavePointStatement),
        Some(statement) => Ok(statement),
        None => Ok(begin_ansi_transaction_sql(depth)),
    }
}

pub fn begin_ansi_transaction_sql(depth: usize) -> SqlStr {
    if depth == 0 {
        "BEGIN".into_sql_str()
    } else {
        AssertSqlSafe(format!("SAVEPOINT _tideorm_savepoint_{depth}")).into_sql_str()
    }
}

pub fn commit_ansi_transaction_sql(depth: usize) -> SqlStr {
    if depth == 1 {
        "COMMIT".into_sql_str()
    } else {
        AssertSqlSafe(format!(
            "RELEASE SAVEPOINT _tideorm_savepoint_{}",
            depth - 1
        ))
        .into_sql_str()
    }
}

/// Roll back the level at `depth`. A savepoint is released too: `ROLLBACK
/// TO` keeps it defined, and the next level opened there would define
/// another on top of it.
pub fn rollback_ansi_transaction_sql(depth: usize) -> SqlStr {
    if depth == 1 {
        "ROLLBACK".into_sql_str()
    } else {
        let name = depth - 1;
        AssertSqlSafe(format!(
            "ROLLBACK TO SAVEPOINT _tideorm_savepoint_{name}; \
             RELEASE SAVEPOINT _tideorm_savepoint_{name}"
        ))
        .into_sql_str()
    }
}

#[cfg(test)]
#[path = "../tests/unit/transaction_tests.rs"]
mod tests;
