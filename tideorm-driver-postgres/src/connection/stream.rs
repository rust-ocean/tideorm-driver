use std::collections::BTreeMap;
use std::ops::{ControlFlow, Deref, DerefMut};
use std::str::FromStr;

use driver_core::bytes::Buf;
use driver_core::transaction::TransactionState;
use futures_channel::mpsc::UnboundedSender;
use futures_util::SinkExt;
use log::Level;

use crate::connection::tls::MaybeUpgradeTls;
use crate::error::{DatabaseError, Error, ErrorKind};
use crate::message::{
    BackendMessage, BackendMessageFormat, EncodeMessage, FrontendMessage, Notice, Notification,
    ParameterStatus, ReadyForQuery, ReceivedMessage, TransactionStatus,
};
use crate::net::{self, BufferedSocket, Socket};
use crate::{PgConnectOptions, PgDatabaseError, PgSeverity};

// the stream is a separate type from the connection to uphold the invariant where an instantiated
// [PgConnection] is a **valid** connection to postgres

// when a new connection is asked for, we work directly on the [PgStream] type until the
// connection is fully established

// in other words, `self` in any PgConnection method is a live connection to postgres that
// is fully prepared to receive queries

pub struct PgStream {
    // A trait object is okay here as the buffering amortizes the overhead of both the dynamic
    // function call as well as the syscall.
    inner: BufferedSocket<Box<dyn Socket>>,

    // buffer of unreceived notification messages from `PUBLISH`
    // this is set when creating a PgListener and only written to if that listener is
    // re-used for query execution in-between receiving messages
    pub(crate) notifications: Option<UnboundedSender<Notification>>,

    pub(crate) parameter_statuses: BTreeMap<String, String>,

    pub(crate) server_version_num: Option<u32>,
    pub(crate) transaction_state: TransactionState,
    /// Set by a deadlock, which aborts the whole transaction even inside a
    /// savepoint; cleared as the next transaction begins.
    pub(crate) deadlocked: bool,
}

impl PgStream {
    pub(super) async fn connect(options: &PgConnectOptions) -> Result<Self, Error> {
        let socket_result = match options.fetch_socket() {
            Some(ref path) => net::connect_uds(path, MaybeUpgradeTls(options)).await?,
            None => net::connect_tcp(&options.host, options.port, MaybeUpgradeTls(options)).await?,
        };

        let socket = socket_result?;

        Ok(Self {
            inner: BufferedSocket::new(socket),
            notifications: None,
            parameter_statuses: BTreeMap::default(),
            server_version_num: None,
            transaction_state: TransactionState::Idle,
            deadlocked: false,
        })
    }

    /// A stream that reads `input`.
    #[cfg(test)]
    pub(crate) fn scripted(input: Vec<u8>) -> Self {
        Self {
            inner: BufferedSocket::new(Box::new(driver_core::net::ScriptedSocket::new(input))),
            notifications: None,
            parameter_statuses: BTreeMap::default(),
            server_version_num: None,
            transaction_state: TransactionState::Idle,
            deadlocked: false,
        }
    }

    #[inline(always)]
    pub(crate) fn write_msg(&mut self, message: impl FrontendMessage) -> Result<(), Error> {
        self.write(EncodeMessage(message))
    }

    pub(crate) async fn send<T>(&mut self, message: T) -> Result<(), Error>
    where
        T: FrontendMessage,
    {
        self.write_msg(message)?;
        self.flush().await?;
        Ok(())
    }

    // Expect a specific type and format
    pub(crate) async fn recv_expect<B: BackendMessage>(&mut self) -> Result<B, Error> {
        self.recv().await?.decode()
    }

    pub(crate) async fn recv_unchecked(&mut self) -> Result<ReceivedMessage, Error> {
        // NOTE: to not break everything, this should be cancel-safe;
        // DO NOT modify `buf` unless a full message has been read
        let message = self
            .inner
            .try_read(|buf| {
                // all packets in postgres start with a 5-byte header
                // this header contains the message type and the total length of the message
                let Some(mut header) = buf.get(..5) else {
                    return Ok(ControlFlow::Continue(5));
                };

                let format = BackendMessageFormat::try_from_u8(header.get_u8())?;

                let message_len = header.get_u32() as usize;

                // the length counts its own 4 bytes
                if message_len < 4 {
                    return Err(err_protocol!("message length {message_len} is below 4"));
                }

                let expected_len = message_len
                    .checked_add(1)
                    // this shouldn't really happen but is mostly a sanity check
                    .ok_or_else(|| {
                        err_protocol!("message_len + 1 overflows usize: {message_len}")
                    })?;

                if buf.len() < expected_len {
                    return Ok(ControlFlow::Continue(expected_len));
                }

                // `buf` SHOULD NOT be modified ABOVE this line

                // pop off the format code since it's not counted in `message_len`
                buf.advance(1);

                // consume the message, including the length prefix
                let mut contents = buf.split_to(message_len).freeze();

                // cut off the length prefix
                contents.advance(4);

                Ok(ControlFlow::Break(ReceivedMessage { format, contents }))
            })
            .await?;
        if message.format == BackendMessageFormat::ReadyForQuery {
            self.transaction_state =
                match ReadyForQuery::decode_body(message.contents.clone())?.transaction_status {
                    TransactionStatus::Idle => TransactionState::Idle,
                    TransactionStatus::Transaction => TransactionState::Active,
                    TransactionStatus::Error => TransactionState::Failed,
                };
        }
        Ok(message)
    }

    // Get the next message from the server
    // May wait for more data from the server
    pub(crate) async fn recv(&mut self) -> Result<ReceivedMessage, Error> {
        loop {
            let message = self.recv_unchecked().await?;

            match message.format {
                BackendMessageFormat::ErrorResponse => {
                    // An error returned from the database server.
                    let error = message.decode::<PgDatabaseError>()?;
                    self.deadlocked |= error.kind() == ErrorKind::Deadlock;
                    return Err(error.into());
                }

                BackendMessageFormat::NotificationResponse => {
                    if let Some(buffer) = &mut self.notifications {
                        let notification: Notification = message.decode()?;
                        let _ = buffer.send(notification).await;

                        continue;
                    }
                }

                BackendMessageFormat::ParameterStatus => {
                    // informs the frontend about the current (initial)
                    // setting of backend parameters

                    // The connection starts with `client_encoding` UTF8 and
                    // `DateStyle` ISO; a later `SET` of either is only recorded.
                    let ParameterStatus { name, value } = message.decode()?;

                    match name.as_str() {
                        "server_version" => {
                            self.server_version_num = parse_server_version(&value);
                        }
                        _ => {
                            self.parameter_statuses.insert(name, value);
                        }
                    }

                    continue;
                }

                BackendMessageFormat::NoticeResponse => {
                    let notice: Notice = message.decode()?;

                    let (log_level, tracing_level) = match notice.severity() {
                        PgSeverity::Fatal | PgSeverity::Panic | PgSeverity::Error => {
                            (Level::Error, tracing::Level::ERROR)
                        }
                        PgSeverity::Warning => (Level::Warn, tracing::Level::WARN),
                        PgSeverity::Notice => (Level::Info, tracing::Level::INFO),
                        PgSeverity::Debug => (Level::Debug, tracing::Level::DEBUG),
                        PgSeverity::Info | PgSeverity::Log => (Level::Trace, tracing::Level::TRACE),
                    };

                    let log_is_enabled = log::log_enabled!(
                        target: "tideorm::driver::postgres::notice",
                        log_level
                    ) || driver_core::private_tracing_dynamic_enabled!(
                        target: "tideorm::driver::postgres::notice",
                        tracing_level
                    );
                    if log_is_enabled {
                        driver_core::private_tracing_dynamic_event!(
                            target: "tideorm::driver::postgres::notice",
                            tracing_level,
                            message = notice.message()
                        );
                    }

                    continue;
                }

                _ => {}
            }

            return Ok(message);
        }
    }
}

impl Deref for PgStream {
    type Target = BufferedSocket<Box<dyn Socket>>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for PgStream {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// reference:
// https://github.com/postgres/postgres/blob/6feebcb6b44631c3dc435e971bd80c2dd218a5ab/src/interfaces/libpq/fe-exec.c#L1030-L1065
fn parse_server_version(s: &str) -> Option<u32> {
    let mut parts = Vec::<u32>::with_capacity(3);

    let mut from = 0;
    let mut chs = s.char_indices().peekable();
    while let Some((i, ch)) = chs.next() {
        match ch {
            '.' => {
                if let Ok(num) = u32::from_str(&s[from..i]) {
                    parts.push(num);
                    from = i + 1;
                } else {
                    break;
                }
            }
            _ if ch.is_ascii_digit() => {
                if chs.peek().is_none() {
                    if let Ok(num) = u32::from_str(&s[from..]) {
                        parts.push(num);
                    }
                    break;
                }
            }
            _ => {
                if let Ok(num) = u32::from_str(&s[from..i]) {
                    parts.push(num);
                }
                break;
            }
        };
    }

    // widened: the server's numbers can overflow a `u32` once scaled
    let version_num = match *parts.as_slice() {
        [major, minor, rev] => (100 * u64::from(major) + u64::from(minor)) * 100 + u64::from(rev),
        [major, minor] if major >= 10 => 100 * 100 * u64::from(major) + u64::from(minor),
        [major, minor] => (100 * u64::from(major) + u64::from(minor)) * 100,
        [major] => 100 * 100 * u64::from(major),
        _ => return None,
    };

    u32::try_from(version_num).ok()
}

#[cfg(test)]
#[path = "../../tests/unit/connection_stream_tests.rs"]
mod tests;
