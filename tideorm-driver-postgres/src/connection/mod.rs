use std::collections::BTreeMap;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use crate::HashMap;

use crate::common::StatementCache;
use crate::error::Error;
use crate::ext::ustr::UStr;
use crate::io::StatementId;
use crate::message::{BackendMessageFormat, CopyFail, Query, ReadyForQuery, Terminate};
use crate::statement::PgStatementMetadata;
use crate::types::Oid;
use crate::{PgConnectOptions, PgTypeInfo, Postgres};

pub(crate) use driver_core::connection::*;

pub use self::stream::PgStream;

mod establish;
mod executor;
mod prepare;
mod resolve;
mod sasl;
mod stream;
mod tls;

/// A connection to a PostgreSQL database.
///
/// See [`PgConnectOptions`] for connection URL reference.
pub struct PgConnection {
    pub(crate) inner: Box<PgConnectionInner>,
}

pub struct PgConnectionInner {
    // underlying TCP or UDS stream,
    // wrapped in a potentially TLS stream,
    // wrapped in a buffered stream
    pub(crate) stream: PgStream,

    // process id of this backend
    // used to send cancel requests
    #[allow(dead_code)]
    process_id: u32,

    // secret key of this backend
    // used to send cancel requests
    #[allow(dead_code)]
    secret_key: u32,

    // sequence of statement IDs for use in preparing statements
    // in PostgreSQL, the statement is prepared to a user-supplied identifier
    next_statement_id: StatementId,

    // cache statement by query string to the id and columns
    cache_statement: StatementCache<(StatementId, Arc<PgStatementMetadata>)>,

    // statements evicted from the cache and not closed on the server yet
    closing: Vec<StatementId>,

    // a named statement whose Parse is in flight; if its prepare fails or is
    // dropped before reading the server's answer, the next prepare closes it
    parsing: Option<StatementId>,

    // cache user-defined types by id <-> info
    cache_type_info: HashMap<Oid, PgTypeInfo>,
    cache_type_oid: HashMap<UStr, Oid>,
    cache_elem_type_to_array: HashMap<Oid, Oid>,
    cache_table_data: HashMap<Oid, TableData>,

    // number of ReadyForQuery messages that we are currently expecting
    pub(crate) pending_ready_for_query_count: usize,

    // depth of the transactions and savepoints this connection has begun
    pub(crate) transaction_depth: usize,
}

pub(crate) struct TableData {
    table_name: Arc<str>,
    /// Attribute number -> name.
    columns: BTreeMap<i16, Arc<str>>,
}

impl PgConnection {
    /// the version number of the server in `libpq` format
    pub fn server_version_num(&self) -> Option<u32> {
        self.inner.stream.server_version_num
    }

    // will return when the connection is ready for another query
    pub(crate) async fn wait_until_ready(&mut self) -> Result<(), Error> {
        if !self.inner.stream.write_buffer_mut().is_empty() {
            self.inner.stream.flush().await?;
        }

        while self.inner.pending_ready_for_query_count > 0 {
            let message = self.inner.stream.recv().await?;

            match message.format {
                BackendMessageFormat::ReadyForQuery => self.handle_ready_for_query()?,
                // a `COPY .. FROM STDIN` whose caller went away before sending any data:
                // the server waits for the data until the copy ends
                BackendMessageFormat::CopyInResponse => {
                    self.inner
                        .stream
                        .send(CopyFail::new("COPY abandoned before its data was sent"))
                        .await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub(crate) async fn recv_ready_for_query(&mut self) -> Result<(), Error> {
        let _: ReadyForQuery = self.inner.stream.recv_expect().await?;

        self.inner.pending_ready_for_query_count -= 1;

        Ok(())
    }

    /// Counts a `ReadyForQuery` the stream has received; the stream already
    /// read the transaction status from it.
    #[inline(always)]
    fn handle_ready_for_query(&mut self) -> Result<(), Error> {
        self.inner.pending_ready_for_query_count = self
            .inner
            .pending_ready_for_query_count
            .checked_sub(1)
            .ok_or_else(|| err_protocol!("received more ReadyForQuery messages than expected"))?;

        Ok(())
    }

    /// Queue a simple query (not prepared) to execute the next time this connection is used.
    ///
    /// Used for rolling back transactions and releasing advisory locks.
    #[inline(always)]
    pub(crate) fn queue_simple_query(&mut self, query: &str) -> Result<(), Error> {
        self.inner.stream.write_msg(Query(query))?;
        self.inner.pending_ready_for_query_count += 1;

        Ok(())
    }

    pub(crate) fn in_transaction(&self) -> bool {
        self.inner.stream.transaction_state == driver_core::transaction::TransactionState::Active
    }
}

impl Debug for PgConnection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgConnection").finish()
    }
}

impl Connection for PgConnection {
    type Database = Postgres;

    type Options = PgConnectOptions;

    async fn close(mut self) -> Result<(), Error> {
        // The normal, graceful termination procedure is that the frontend sends a Terminate
        // message and immediately closes the connection.

        // On receipt of this message, the backend closes the
        // connection and terminates.
        self.inner.stream.send(Terminate).await?;
        self.inner.stream.shutdown().await?;

        Ok(())
    }

    async fn close_hard(mut self) -> Result<(), Error> {
        self.inner.stream.shutdown().await?;

        Ok(())
    }

    async fn ping(&mut self) -> Result<(), Error> {
        // The simplest call-and-response that's possible.
        self.write_sync();
        self.wait_until_ready().await
    }

    fn cached_statements_size(&self) -> usize {
        self.inner.cache_statement.len()
    }

    async fn clear_cached_statements(&mut self) -> Result<(), Error> {
        self.inner.cache_type_oid.clear();

        self.wait_until_ready().await?;

        while let Some((id, _)) = self.inner.cache_statement.remove_lru() {
            self.inner.closing.push(id);
        }
        self.close_evicted().await
    }

    fn shrink_buffers(&mut self) {
        self.inner.stream.shrink_buffers();
    }

    #[doc(hidden)]
    fn flush(&mut self) -> impl Future<Output = Result<(), Error>> + Send + '_ {
        self.wait_until_ready()
    }
}

// Implement `AsMut<Self>` so that `PgConnection` can be wrapped in
// a `PgAdvisoryLockGuard`.
impl AsMut<PgConnection> for PgConnection {
    fn as_mut(&mut self) -> &mut PgConnection {
        self
    }
}
