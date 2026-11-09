use driver_core::database::Database;
use driver_core::sql_str::SqlStr;

use crate::error::Error;
use crate::executor::Executor;

use crate::{PgConnection, Postgres};

pub(crate) use driver_core::transaction::*;

/// Implementation of [`TransactionManager`] for PostgreSQL.
pub struct PgTransactionManager;

impl TransactionManager for PgTransactionManager {
    type Database = Postgres;

    async fn begin(conn: &mut PgConnection, statement: Option<SqlStr>) -> Result<(), Error> {
        let depth = conn.inner.transaction_depth;

        let statement = begin_statement(depth, statement)?;

        // Read what earlier statements left pending first, so the reply read
        // below is this statement's own and the state it starts from current.
        conn.wait_until_ready().await?;
        if depth == 0 {
            conn.inner.stream.deadlocked = false;
        }
        let undo = (conn.inner.stream.transaction_state != TransactionState::Failed)
            .then(|| rollback_ansi_transaction_sql(depth + 1));
        conn.queue_simple_query(statement.as_str())?;
        let mut opening = Opening { conn, undo };
        let answered = opening.conn.wait_until_ready().await;
        // Answered: the level opened, or the server refused it and none did.
        opening.undo = None;
        answered?;
        if !opening.conn.in_transaction() {
            return Err(Error::BeginFailed);
        }
        opening.conn.inner.transaction_depth += 1;

        Ok(())
    }

    async fn commit(conn: &mut PgConnection) -> Result<(), Error> {
        if conn.inner.transaction_depth > 0 {
            conn.execute(commit_ansi_transaction_sql(conn.inner.transaction_depth))
                .await?;

            conn.inner.transaction_depth -= 1;
        }

        Ok(())
    }

    async fn rollback(conn: &mut PgConnection) -> Result<(), Error> {
        let depth = conn.inner.transaction_depth;
        if depth > 0 {
            conn.execute(rollback_ansi_transaction_sql(depth)).await?;
            conn.inner.transaction_depth = depth - 1;
            return Ok(());
        }
        // No transaction of ours is open, but the server may still hold one
        // (a raw `BEGIN`); its state is current once the queued replies are read.
        conn.wait_until_ready().await?;
        if Self::get_transaction_state(conn) != TransactionState::Idle {
            conn.execute(rollback_ansi_transaction_sql(1)).await?;
        }

        Ok(())
    }

    fn start_rollback(conn: &mut PgConnection) {
        if conn.inner.transaction_depth > 0 {
            conn.queue_simple_query(
                rollback_ansi_transaction_sql(conn.inner.transaction_depth).as_str(),
            )
            .expect("BUG: Rollback query somehow too large for protocol");

            conn.inner.transaction_depth -= 1;
        }
    }

    /// The server's state, except that a deadlock aborts the transaction the
    /// driver counts open, whichever savepoint it rolls back to.
    fn get_transaction_state(conn: &PgConnection) -> TransactionState {
        if conn.inner.transaction_depth > 0 && conn.inner.stream.deadlocked {
            TransactionState::Aborted
        } else {
            conn.inner.stream.transaction_state
        }
    }

    fn get_transaction_depth(conn: &<Self::Database as Database>::Connection) -> usize {
        conn.inner.transaction_depth
    }
}

/// A level `begin` sent whose answer it has not read. A begin dropped while
/// it waits undoes that level alone, which leaves an enclosing transaction as
/// it was: rolling back at the enclosing depth would end that one instead.
struct Opening<'c> {
    conn: &'c mut PgConnection,
    /// What undoes the level, a savepoint together with its name; none once
    /// answered, or when the enclosing transaction has failed, where the
    /// level cannot open.
    undo: Option<SqlStr>,
}

impl Drop for Opening<'_> {
    fn drop(&mut self) {
        if let Some(undo) = self.undo.take() {
            self.conn
                .queue_simple_query(undo.as_str())
                .expect("BUG: Rollback query somehow too large for protocol");
        }
    }
}
