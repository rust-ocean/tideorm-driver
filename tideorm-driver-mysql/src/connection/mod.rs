use std::fmt::{self, Debug, Formatter};
use std::future::Future;

pub(crate) use driver_core::connection::*;
pub(crate) use stream::{MySqlStream, Waiting};

use crate::collation::Collation;
use crate::common::StatementCache;
use crate::error::Error;
use crate::protocol::response::Status;
use crate::protocol::Command;
use crate::statement::MySqlStatementMetadata;
use crate::{MySql, MySqlConnectOptions};

mod auth;
mod establish;
mod executor;
mod stream;
mod tls;

const MAX_PACKET_SIZE: u32 = 1024;

/// The charset parameter sent in the `Protocol::HandshakeResponse41` packet.
///
/// This becomes the default if `set_names = false`,
/// and also ensures that any error messages returned before `SET NAMES` are encoded correctly.
#[allow(clippy::cast_possible_truncation)]
const INITIAL_CHARSET: u8 = Collation::UTF8MB4_GENERAL_CI.0 as u8;

/// A connection to a MySQL database.
pub struct MySqlConnection {
    pub(crate) inner: Box<MySqlConnectionInner>,
}

pub(crate) struct MySqlConnectionInner {
    // underlying TCP stream,
    // wrapped in a potentially TLS stream,
    // wrapped in a buffered stream
    pub(crate) stream: MySqlStream,

    // depth of the transactions and savepoints this connection has begun
    pub(crate) transaction_depth: usize,

    // cache by query string to the statement id and metadata
    cache_statement: StatementCache<(u32, MySqlStatementMetadata)>,

    // whether the server's handshake named MariaDB
    mariadb: bool,
}

impl MySqlConnection {
    /// Whether the server is MariaDB, as its handshake said: MariaDB speaks
    /// MySQL's protocol, but renders some SQL differently.
    pub fn is_mariadb(&self) -> bool {
        self.inner.mariadb
    }

    pub(crate) fn in_transaction(&self) -> bool {
        self.inner
            .stream
            .status_flags
            .intersects(Status::SERVER_STATUS_IN_TRANS)
    }
}

impl Debug for MySqlConnection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlConnection").finish()
    }
}

impl Connection for MySqlConnection {
    type Database = MySql;

    type Options = MySqlConnectOptions;

    async fn close(mut self) -> Result<(), Error> {
        self.inner.stream.send_packet(Command::Quit).await?;
        self.inner.stream.shutdown().await?;

        Ok(())
    }

    async fn close_hard(mut self) -> Result<(), Error> {
        self.inner.stream.shutdown().await?;
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), Error> {
        self.inner.stream.wait_until_ready().await?;
        // Awaited as a reply, so a ping cancelled before its OK arrives
        // leaves that OK to be read before the next statement's reply.
        self.inner.stream.write_command(Command::Ping)?;
        self.inner.stream.waiting.push_back(Waiting::Result);
        self.inner.stream.wait_until_ready().await
    }

    #[doc(hidden)]
    fn flush(&mut self) -> impl Future<Output = Result<(), Error>> + Send + '_ {
        self.inner.stream.wait_until_ready()
    }

    fn cached_statements_size(&self) -> usize {
        self.inner.cache_statement.len()
    }

    async fn clear_cached_statements(&mut self) -> Result<(), Error> {
        while let Some((statement_id, _)) = self.inner.cache_statement.remove_lru() {
            self.inner
                .stream
                .send_packet(Command::CloseStatement(statement_id))
                .await?;
        }

        Ok(())
    }

    fn shrink_buffers(&mut self) {
        self.inner.stream.shrink_buffers();
    }
}
