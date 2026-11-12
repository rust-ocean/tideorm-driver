//! Preparing a statement in a round trip of its own, for `prepare_with`, and closing the
//! statements the cache evicted.

use crate::error::Error;
use crate::io::StatementId;
use crate::message::{
    BackendMessageFormat, Close, ParameterDescription, ParseComplete, RowDescription,
};
use crate::statement::PgStatementMetadata;
use crate::{PgConnection, PgTypeInfo};
use std::sync::Arc;

async fn prepare(
    conn: &mut PgConnection,
    sql: &str,
    arg_types: &[PgTypeInfo],
    metadata: Option<Arc<PgStatementMetadata>>,
    persistent: bool,
    resolve_column_origin: bool,
) -> Result<(StatementId, Arc<PgStatementMetadata>), Error> {
    let id = if persistent {
        let id = conn.inner.next_statement_id;
        conn.inner.next_statement_id = id.next();
        id
    } else {
        StatementId::UNNAMED
    };

    // build a list of type OIDs to send to the database in the PARSE command
    // we have not yet started the query sequence, so we are *safe* to cleanly make
    // additional queries here to get any missing OIDs
    let param_types = conn.resolve_types(arg_types).await?;

    // flush and wait until we are re-ready
    conn.wait_until_ready().await?;

    if persistent {
        conn.inner.parsing = Some(id);
    }

    // next we send the PARSE command to the server
    // with its columns and parameters, unless the caller knows them
    conn.write_parse(id, &param_types, sql, metadata.is_none())?;

    // we ask for the server to immediately send us the result of the PARSE command
    conn.write_sync();
    conn.inner.stream.flush().await?;

    // indicates that the SQL query string is now successfully parsed and has semantic validity
    conn.inner.stream.recv_expect::<ParseComplete>().await?;

    let metadata = if let Some(metadata) = metadata {
        // each SYNC produces one READY FOR QUERY
        conn.recv_ready_for_query().await?;
        conn.inner.parsing = None;

        // we already have metadata
        metadata
    } else {
        let parameters = recv_desc_params(conn).await?;

        let row_desc = recv_desc_rows(conn).await?;

        // each SYNC produces one READY FOR QUERY
        conn.recv_ready_for_query().await?;
        // Resolving the types may run statements of its own, whose prepare would
        // close a statement still marked as parsing.
        conn.inner.parsing = None;

        let resolved = async {
            let metadata = conn
                .resolve_statement_metadata::<true>(
                    Some(parameters),
                    row_desc,
                    resolve_column_origin,
                )
                .await?;

            // ensure that if we did fetch custom data, we wait until we are fully ready before
            // continuing
            conn.wait_until_ready().await?;

            Ok::<_, Error>(metadata)
        }
        .await;

        // shortcut: a prepare dropped while it resolves types (only types or column
        // origins the connection has not cached yet) leaves its statement open until
        // the connection closes; track it past this point if such leaks show up.
        match resolved {
            Ok(metadata) => metadata,
            Err(error) => {
                if persistent {
                    conn.inner.closing.push(id);
                }
                return Err(error);
            }
        }
    };

    Ok((id, metadata))
}

async fn recv_desc_params(conn: &mut PgConnection) -> Result<ParameterDescription, Error> {
    conn.inner.stream.recv_expect().await
}

async fn recv_desc_rows(conn: &mut PgConnection) -> Result<Option<RowDescription>, Error> {
    let rows: Option<RowDescription> = match conn.inner.stream.recv().await? {
        // describes the rows that will be returned when the statement is eventually executed
        message if message.format == BackendMessageFormat::RowDescription => {
            Some(message.decode()?)
        }

        // no data would be returned if this statement was executed
        message if message.format == BackendMessageFormat::NoData => None,

        message => {
            return Err(err_protocol!(
                "expecting RowDescription or NoData but received {:?}",
                message.format
            ));
        }
    };

    Ok(rows)
}

impl PgConnection {
    // wait for CloseComplete to indicate a statement was closed
    pub(super) async fn wait_for_close_complete(&mut self, mut count: usize) -> Result<(), Error> {
        // we need to wait for the [CloseComplete] to be returned from the server
        while count > 0 {
            match self.inner.stream.recv().await? {
                message if message.format == BackendMessageFormat::PortalSuspended => {
                    // there was an open portal
                    // this can happen if the last time a statement was used it was not fully executed
                }

                message if message.format == BackendMessageFormat::CloseComplete => {
                    // successfully closed the statement (and freed up the server resources)
                    count -= 1;
                }

                message => {
                    return Err(err_protocol!(
                        "expecting PortalSuspended or CloseComplete but received {:?}",
                        message.format
                    ));
                }
            }
        }

        Ok(())
    }

    pub(super) async fn get_or_prepare(
        &mut self,
        sql: &str,
        parameters: &[PgTypeInfo],
        persistent: bool,
        // optional metadata that was provided by the user, this means they are reusing
        // a statement object
        metadata: Option<Arc<PgStatementMetadata>>,
        resolve_column_origin: bool,
    ) -> Result<(StatementId, Arc<PgStatementMetadata>), Error> {
        // a statement an earlier prepare left behind closes with the next execution
        if let Some(id) = self.inner.parsing.take() {
            self.inner.closing.push(id);
        }
        if let Some(statement) = self.inner.cache_statement.get_mut(sql) {
            return Ok((*statement).clone());
        }
        // Without a cache a named statement would never be closed.
        let persistent = persistent && self.inner.cache_statement.is_enabled();

        let statement = prepare(
            self,
            sql,
            parameters,
            metadata,
            persistent,
            resolve_column_origin,
        )
        .await?;

        if persistent {
            self.remember_statement(sql, statement.0, &statement.1);
        }

        Ok(statement)
    }

    /// Caches a statement the server has parsed; one the cache evicts is closed with the next
    /// execution's messages, not a round trip of its own.
    pub(super) fn remember_statement(
        &mut self,
        sql: &str,
        id: StatementId,
        metadata: &Arc<PgStatementMetadata>,
    ) {
        self.inner.parsing = None;
        if let Some((evicted, _)) = self
            .inner
            .cache_statement
            .insert(sql, (id, Arc::clone(metadata)))
        {
            self.inner.closing.push(evicted);
        }
    }

    /// Close the statements evicted from the cache that no execution has
    /// closed yet, in a round trip of their own.
    pub(super) async fn close_evicted(&mut self) -> Result<(), Error> {
        let closing = std::mem::take(&mut self.inner.closing);
        if closing.is_empty() {
            return Ok(());
        }
        for &id in &closing {
            self.inner.stream.write_msg(Close::Statement(id))?;
        }
        self.write_sync();
        self.inner.stream.flush().await?;

        self.wait_for_close_complete(closing.len()).await?;
        self.recv_ready_for_query().await
    }
}
