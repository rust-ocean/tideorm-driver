use driver_core::sql_str::SqlStr;

use crate::connection::Waiting;
use crate::error::Error;
use crate::executor::Executor;
use crate::protocol::Command;
use crate::{MySql, MySqlConnection};

pub(crate) use driver_core::transaction::*;

/// Implementation of [`TransactionManager`] for MySQL.
pub struct MySqlTransactionManager;

impl TransactionManager for MySqlTransactionManager {
    type Database = MySql;

    async fn begin(conn: &mut MySqlConnection, statement: Option<SqlStr>) -> Result<(), Error> {
        let depth = conn.inner.transaction_depth;

        let statement = begin_statement(depth, statement)?;
        // Read what earlier statements left pending first, so the reply read
        // below is this statement's own.
        conn.inner.stream.wait_until_ready().await?;
        if depth == 0 {
            conn.inner.stream.deadlocked = false;
        }
        // After a deadlock the server holds no transaction, so a savepoint
        // cannot open there.
        let undo = (!aborted(conn)).then_some(depth + 1);
        queue_query(conn, statement.as_str())?;
        let mut opening = Opening { conn, undo };
        let answered = opening.conn.inner.stream.wait_until_ready().await;
        // Answered: the level opened, or the server refused it and none did.
        opening.undo = None;
        answered?;
        if !opening.conn.in_transaction() {
            return Err(Error::BeginFailed);
        }
        opening.conn.inner.transaction_depth += 1;

        Ok(())
    }

    async fn commit(conn: &mut MySqlConnection) -> Result<(), Error> {
        let depth = conn.inner.transaction_depth;

        if depth > 0 {
            conn.execute(commit_ansi_transaction_sql(depth)).await?;
            conn.inner.transaction_depth = depth - 1;
        }

        Ok(())
    }

    async fn rollback(conn: &mut MySqlConnection) -> Result<(), Error> {
        let depth = conn.inner.transaction_depth;
        if depth > 0 {
            // The server rolled an aborted transaction back already, and its
            // savepoints with it.
            if !aborted(conn) {
                conn.execute(rollback_ansi_transaction_sql(depth)).await?;
            }
            conn.inner.transaction_depth = depth - 1;
            return Ok(());
        }
        // No transaction of ours is open, but the server may still hold one
        // (a raw `BEGIN`); its state is current once the queued replies are read.
        conn.inner.stream.wait_until_ready().await?;
        if conn.in_transaction() {
            conn.execute(rollback_ansi_transaction_sql(1)).await?;
        }

        Ok(())
    }

    fn start_rollback(conn: &mut MySqlConnection) {
        let depth = conn.inner.transaction_depth;

        if depth > 0 && !aborted(conn) {
            queue_query(conn, rollback_ansi_transaction_sql(depth).as_str())
                .expect("BUG: unexpected error queueing ROLLBACK");
        }
        conn.inner.transaction_depth = depth.saturating_sub(1);
    }

    fn get_transaction_state(conn: &MySqlConnection) -> TransactionState {
        if aborted(conn) {
            TransactionState::Aborted
        } else if conn.in_transaction() {
            TransactionState::Active
        } else {
            TransactionState::Idle
        }
    }

    fn get_transaction_depth(conn: &MySqlConnection) -> usize {
        conn.inner.transaction_depth
    }
}

/// Whether a deadlock rolled back the transaction the driver counts open.
fn aborted(conn: &MySqlConnection) -> bool {
    conn.inner.transaction_depth > 0 && conn.inner.stream.deadlocked
}

/// Send `sql` with the next flush; the next `wait_until_ready` reads its reply.
fn queue_query(conn: &mut MySqlConnection, sql: &str) -> Result<(), Error> {
    conn.inner.stream.write_command(Command::Query(sql))?;
    conn.inner.stream.waiting.push_back(Waiting::Result);
    Ok(())
}

/// A level `begin` sent whose answer it has not read. A begin dropped while
/// it waits undoes that level alone, which leaves an enclosing transaction as
/// it was; otherwise the server would hold a transaction the driver does not
/// count, and the connection's later statements would run inside it.
struct Opening<'c> {
    conn: &'c mut MySqlConnection,
    /// The depth of the level to undo; none once answered, or where the level
    /// cannot open.
    undo: Option<usize>,
}

impl Drop for Opening<'_> {
    fn drop(&mut self) {
        if let Some(depth) = self.undo.take() {
            queue_query(self.conn, rollback_ansi_transaction_sql(depth).as_str())
                .expect("BUG: unexpected error queueing ROLLBACK");
        }
    }
}
