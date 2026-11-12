use crate::error::Error;
use crate::executor::{Execute, Executor};
use crate::io::{PortalId, StatementId};
use crate::message::{self, BackendMessageFormat, Bind, Close, CommandComplete, DataRow, Parse};
use crate::statement::PgStatementMetadata;
use crate::types::Oid;
use crate::{
    statement::PgStatement, PgArguments, PgConnection, PgQueryResult, PgRow, PgTypeInfo,
    PgValueFormat, Postgres,
};
use driver_core::arguments::Arguments;
use driver_core::sql_str::SqlStr;
use driver_core::Either;
use futures_core::Stream;
use futures_util::TryStreamExt;
use std::collections::VecDeque;
use std::{future::Future, pin::pin, sync::Arc};

impl PgConnection {
    #[inline(always)]
    pub(crate) fn write_sync(&mut self) {
        self.inner
            .stream
            .write_msg(message::Sync)
            .expect("BUG: Sync should not be too big for protocol");

        // all SYNC messages will return a ReadyForQuery
        self.inner.pending_ready_for_query_count += 1;
    }

    /// Parse `sql` as `statement` with `param_types`, asking for its description when
    /// `describe`.
    pub(super) fn write_parse(
        &mut self,
        statement: StatementId,
        param_types: &[Oid],
        sql: &str,
        describe: bool,
    ) -> Result<(), Error> {
        self.inner.stream.write_msg(Parse {
            param_types,
            query: sql,
            statement,
        })?;
        if describe {
            self.inner
                .stream
                .write_msg(message::Describe::Statement(statement))?;
        }
        Ok(())
    }

    /// Bind `params` to `statement` and execute it in the unnamed portal.
    /// The caller writes the `Sync` that ends the cycle.
    fn write_execution(
        &mut self,
        statement: StatementId,
        num_params: u16,
        params: &[u8],
    ) -> Result<(), Error> {
        self.inner.stream.write_msg(Bind {
            portal: PortalId::UNNAMED,
            statement,
            formats: &[PgValueFormat::Binary],
            num_params,
            params,
            result_formats: &[PgValueFormat::Binary],
        })?;

        // executes the portal up to the passed limit
        // the protocol-level limit acts nearly identically to the `LIMIT` in SQL
        self.inner.stream.write_msg(message::Execute {
            portal: PortalId::UNNAMED,
            // Non-zero limits cause query plan pessimization by disabling parallel workers.
            limit: 0,
        })?;
        // From https://www.postgresql.org/docs/current/protocol-flow.html:
        //
        // "An unnamed portal is destroyed at the end of the transaction, or as
        // soon as the next Bind statement specifying the unnamed portal as
        // destination is issued. (Note that a simple Query message also
        // destroys the unnamed portal."

        // we ask the database server to close the unnamed portal and free the associated resources
        // earlier - after the execution of the current query.
        self.inner
            .stream
            .write_msg(Close::Portal(PortalId::UNNAMED))
    }

    pub(crate) async fn run<'e, 'c: 'e, 'q: 'e>(
        &'c mut self,
        query: SqlStr,
        arguments: Option<PgArguments>,
        persistent: bool,
        metadata_opt: Option<Arc<PgStatementMetadata>>,
    ) -> Result<impl Stream<Item = Result<Either<PgQueryResult, PgRow>, Error>> + 'e, Error> {
        // before we continue, wait until we are "ready" to accept more queries
        self.wait_until_ready().await?;

        // metadata starts out as "nothing"
        let mut metadata = Arc::new(PgStatementMetadata::default());
        // A statement parsed in this round trip is described in it too, ahead of its rows,
        // and a named one is cached once described.
        let mut describing = false;
        let mut parsed = None;

        let format = if let Some(mut arguments) = arguments {
            // Check this before we write anything to the stream.
            //
            // Note: Postgres actually interprets this value as unsigned,
            // making the max number of parameters 65535, not 32767.
            // https://www.postgresql.org/docs/current/limits.html
            let num_params = u16::try_from(arguments.len()).map_err(|_| {
                err_protocol!(
                    "PgConnection::run(): too many arguments for query: {}",
                    arguments.len()
                )
            })?;

            // a statement an earlier prepare left behind closes with this execution
            if let Some(id) = self.inner.parsing.take() {
                self.inner.closing.push(id);
            }

            let cached = self.inner.cache_statement.get_mut(query.as_str()).cloned();
            let (statement, param_types) = if let Some((id, cached)) = cached {
                // patch holes created during encoding
                arguments
                    .apply_patches(self, Some(&cached.parameters))
                    .await?;
                metadata = cached;
                (id, None)
            } else {
                // Without a cache a named statement would never be closed.
                let id = if persistent && self.inner.cache_statement.is_enabled() {
                    let id = self.inner.next_statement_id;
                    self.inner.next_statement_id = id.next();
                    parsed = Some(id);
                    // Room is made now, so the statement this one evicts closes
                    // ahead of its Parse: the server never holds more
                    // statements than the cache.
                    let cache = &mut self.inner.cache_statement;
                    let evicted = if cache.len() == cache.capacity() {
                        cache.remove_lru()
                    } else {
                        None
                    };
                    if let Some((evicted, _)) = evicted {
                        self.inner.closing.push(evicted);
                    }
                    id
                } else {
                    StatementId::UNNAMED
                };
                // Resolving names runs statements of its own, so it comes first.
                let param_types = self.resolve_types(&arguments.types).await?;
                // the statement takes the types it is parsed with
                arguments.apply_patches(self, None).await?;
                (id, Some(param_types))
            };

            // consume messages till `ReadyForQuery` before bind and execute
            self.wait_until_ready().await?;

            // Statements evicted from the cache close ahead of the Bind: an
            // error there skips the messages after it, and a Close is allowed
            // even in a failed transaction.
            for id in std::mem::take(&mut self.inner.closing) {
                self.inner.stream.write_msg(Close::Statement(id))?;
            }
            let written = match param_types {
                Some(param_types) => {
                    // closed with a later execution if no description caches it
                    self.inner.parsing = parsed;
                    describing = metadata_opt.is_none();
                    if let Some(given) = &metadata_opt {
                        metadata = Arc::clone(given);
                    }
                    self.write_parse(statement, &param_types, query.as_str(), describing)
                }
                None => Ok(()),
            }
            .and_then(|()| self.write_execution(statement, num_params, &arguments.buffer));
            // [Sync] asks postgres to process the messages we sent and answer
            // with a [ReadyForQuery] once done; it is written even after a
            // refused write, to end the cycle the Closes began.
            self.write_sync();
            written?;

            // prepared statements are binary
            PgValueFormat::Binary
        } else {
            // Query will trigger a ReadyForQuery
            self.queue_simple_query(query.as_str())?;

            // and unprepared statements are text
            PgValueFormat::Text
        };

        self.inner.stream.flush().await?;

        Ok(try_stream! {
            let mut parameters = None;
            // the rest of a reply read before its statement's types could be looked up
            let mut held = VecDeque::new();
            let mut ended = false;
            loop {
                let message = match held.pop_front() {
                    Some(message) => message,
                    None if ended => break,
                    None => self.inner.stream.recv().await?,
                };

                match message.format {
                    // a statement the caller described itself is cached once parsed
                    BackendMessageFormat::ParseComplete if !describing => {
                        if let Some(id) = parsed.take() {
                            self.remember_statement(query.as_str(), id, &metadata);
                        }
                    }

                    BackendMessageFormat::ParameterDescription if describing => {
                        parameters = Some(message.decode()?);
                    }

                    BackendMessageFormat::RowDescription | BackendMessageFormat::NoData
                        if describing =>
                    {
                        describing = false;
                        let rows = match message.format {
                            BackendMessageFormat::RowDescription => Some(message.decode()?),
                            _ => None,
                        };
                        let parameters = parameters.take();
                        metadata = if self.knows_types(parameters.as_ref(), rows.as_ref()) {
                            self.resolve_statement_metadata::<false>(parameters, rows, false)
                                .await?
                        } else {
                            // Looking types up runs statements of its own, so the rest of
                            // this reply is read first and handed on after.
                            loop {
                                let message = self.inner.stream.recv().await?;
                                if message.format == BackendMessageFormat::ReadyForQuery {
                                    self.handle_ready_for_query()?;
                                    ended = true;
                                    break;
                                }
                                held.push_back(message);
                            }
                            let resolved = self
                                .resolve_statement_metadata::<true>(parameters, rows, false)
                                .await?;
                            self.wait_until_ready().await?;
                            resolved
                        };
                        if let Some(id) = parsed.take() {
                            self.remember_statement(query.as_str(), id, &metadata);
                        }
                    }

                    BackendMessageFormat::BindComplete
                    | BackendMessageFormat::ParseComplete
                    | BackendMessageFormat::ParameterDescription
                    | BackendMessageFormat::NoData
                    // unnamed portal has been closed
                    | BackendMessageFormat::CloseComplete
                    // empty query string passed to an unprepared execute
                    | BackendMessageFormat::EmptyQueryResponse
                    // incomplete query execution has finished
                    | BackendMessageFormat::PortalSuspended
                    => {
                        // harmless messages to ignore
                    }

                    // "Execute phase is always terminated by the appearance of
                    // exactly one of these messages: CommandComplete,
                    // EmptyQueryResponse (if the portal was created from an
                    // empty query string), ErrorResponse, or PortalSuspended"
                    BackendMessageFormat::CommandComplete => {
                        // a SQL command completed normally
                        let cc: CommandComplete = message.decode()?;

                        let rows_affected = cc.rows_affected();
                        r#yield!(Either::Left(PgQueryResult {
                            rows_affected,
                        }));
                    }

                    // Message::ErrorResponse is handled in self.stream.recv()

                    // indicates that a *new* set of rows are about to be returned
                    BackendMessageFormat::RowDescription => {
                        let new_metadata = self.resolve_statement_metadata::<false>(
                            None,
                            Some(message.decode()?),
                            false,
                        ).await?;

                        metadata = new_metadata;
                    }

                    BackendMessageFormat::DataRow => {
                        // one of the set of rows returned by a SELECT, FETCH, etc query
                        let data: DataRow = message.decode()?;
                        let row = PgRow {
                            data,
                            format,
                            metadata: Arc::clone(&metadata),
                        };

                        r#yield!(Either::Right(row));
                    }

                    BackendMessageFormat::ReadyForQuery => {
                        // processing of the query string is complete
                        self.handle_ready_for_query()?;
                        break;
                    }

                    _ => {
                        return Err(err_protocol!(
                            "execute: unexpected message: {:?}",
                            message.format
                        ));
                    }
                }
            }

            Ok(())
        })
    }
}

impl<'c> Executor<'c> for &'c mut PgConnection {
    type Database = Postgres;

    fn fetch_many<'e, 'q, E>(
        self,
        mut query: E,
    ) -> impl Stream<Item = Result<Either<PgQueryResult, PgRow>, Error>> + Send + 'e
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
        'q: 'e,
        E: 'q,
    {
        // False positive: https://github.com/rust-lang/rust-clippy/issues/12560
        #[allow(clippy::map_clone)]
        let metadata = query.statement().map(|s| Arc::clone(&s.metadata));
        let arguments = query.take_arguments().map_err(Error::Encode);
        let persistent = query.persistent();
        let sql = query.sql();

        try_stream! {
            let arguments = arguments?;
            let mut s = pin!(self.run(sql, arguments, persistent, metadata).await?);

            while let Some(v) = s.try_next().await? {
                r#yield!(v);
            }

            Ok(())
        }
    }

    fn fetch_optional<'e, 'q, E>(
        self,
        mut query: E,
    ) -> impl Future<Output = Result<Option<PgRow>, Error>> + Send + 'e
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
        'q: 'e,
        E: 'q,
    {
        // False positive: https://github.com/rust-lang/rust-clippy/issues/12560
        #[allow(clippy::map_clone)]
        let metadata = query.statement().map(|s| Arc::clone(&s.metadata));
        let arguments = query.take_arguments().map_err(Error::Encode);
        let persistent = query.persistent();

        async move {
            let sql = query.sql();
            let arguments = arguments?;
            let mut s = pin!(self.run(sql, arguments, persistent, metadata).await?);

            // With deferred constraints we need to check all responses as we
            // could get a OK response (with uncommitted data), only to get an
            // error response after (when the deferred constraint is actually
            // checked).
            let mut ret = None;
            while let Some(result) = s.try_next().await? {
                match result {
                    Either::Right(r) if ret.is_none() => ret = Some(r),
                    _ => {}
                }
            }
            Ok(ret)
        }
    }

    async fn prepare_with<'e>(
        self,
        sql: SqlStr,
        parameters: &'e [PgTypeInfo],
    ) -> Result<PgStatement, Error>
    where
        'c: 'e,
    {
        self.wait_until_ready().await?;

        let (_, metadata) = self
            .get_or_prepare(sql.as_str(), parameters, true, None, true)
            .await?;
        // no execution follows to carry the Close of an evicted statement
        self.close_evicted().await?;

        Ok(PgStatement { sql, metadata })
    }
}
