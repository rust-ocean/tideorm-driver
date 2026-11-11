//! The messages of PostgreSQL's frontend/backend protocol (version 3.0) that the driver sends
//! and reads.
//!
//! After start-up, every message is a format byte, a big-endian `i32` length that counts itself
//! but not the format byte, and a body:
//! <https://www.postgresql.org/docs/current/protocol-message-formats.html>.

use driver_core::bytes::Bytes;
use driver_core::io::ProtocolEncode;
use std::num::Saturating;

use crate::error::Error;
use crate::io::PgBufMutExt;

mod authentication;
mod backend_key_data;
mod bind;
mod close;
mod command_complete;
mod copy;
mod data_row;
mod describe;
mod execute;
mod notification;
mod parameter_description;
mod parameter_status;
mod parse;
mod password;
mod query;
mod ready_for_query;
mod response;
mod row_description;
mod sasl;
mod ssl_request;
mod startup;

pub use authentication::{Authentication, AuthenticationSasl};
pub use backend_key_data::BackendKeyData;
pub use bind::Bind;
pub use close::Close;
pub use command_complete::CommandComplete;
pub use copy::{CopyData, CopyDone, CopyFail, CopyInResponse, CopyOutResponse, CopyResponseData};
pub use data_row::DataRow;
pub use describe::Describe;
pub use execute::Execute;
pub use notification::Notification;
pub use parameter_description::ParameterDescription;
pub use parameter_status::ParameterStatus;
pub use parse::Parse;
pub use password::Password;
pub use query::Query;
pub use ready_for_query::{ReadyForQuery, TransactionStatus};
pub use response::{Notice, PgSeverity};
pub use row_description::RowDescription;
pub use sasl::{SaslInitialResponse, SaslResponse};
pub use ssl_request::SslRequest;
pub use startup::Startup;

// The two directions get an enum each: they reuse format bytes for different messages (`C` is
// `Close` from the client but `CommandComplete` from the server).

/// The format byte of each message the driver sends.
#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum FrontendMessageFormat {
    Bind = b'B',
    Close = b'C',
    CopyData = b'd',
    CopyDone = b'c',
    CopyFail = b'f',
    Describe = b'D',
    Execute = b'E',
    Parse = b'P',
    /// Every authentication reply shares this byte: a cleartext or MD5 password, and the SASL
    /// messages.
    PasswordPolymorphic = b'p',
    Query = b'Q',
    Sync = b'S',
    Terminate = b'X',
}

/// Declares [`BackendMessageFormat`] from one table of variants and format bytes.
macro_rules! backend_formats {
    ($($format:ident = $byte:literal,)+) => {
        /// The format byte of each message the server can send.
        #[derive(Debug, PartialEq)]
        #[repr(u8)]
        pub enum BackendMessageFormat {
            $($format = $byte,)+
        }

        impl BackendMessageFormat {
            pub fn try_from_u8(v: u8) -> Result<Self, Error> {
                match v {
                    $($byte => Ok(Self::$format),)+
                    _ => Err(err_protocol!("unknown message type: {:?}", v as char)),
                }
            }
        }
    };
}

backend_formats! {
    Authentication = b'R',
    BackendKeyData = b'K',
    BindComplete = b'2',
    CloseComplete = b'3',
    CommandComplete = b'C',
    CopyData = b'd',
    CopyDone = b'c',
    CopyInResponse = b'G',
    CopyOutResponse = b'H',
    DataRow = b'D',
    EmptyQueryResponse = b'I',
    ErrorResponse = b'E',
    NoData = b'n',
    NoticeResponse = b'N',
    NotificationResponse = b'A',
    ParameterDescription = b't',
    ParameterStatus = b'S',
    ParseComplete = b'1',
    PortalSuspended = b's',
    ReadyForQuery = b'Z',
    RowDescription = b'T',
}

/// A message read from the server, its body not yet decoded.
#[derive(Debug)]
pub struct ReceivedMessage {
    pub format: BackendMessageFormat,
    pub contents: Bytes,
}

impl ReceivedMessage {
    /// Decodes the body as a `T`, after checking the message is one.
    #[inline]
    pub fn decode<T>(self) -> Result<T, Error>
    where
        T: BackendMessage,
    {
        if T::FORMAT != self.format {
            return Err(err_protocol!(
                "Postgres protocol error: expected {:?}, got {:?}",
                T::FORMAT,
                self.format
            ));
        }

        T::decode_body(self.contents).map_err(|e| match e {
            Error::Protocol(s) => {
                err_protocol!("Postgres protocol error (reading {:?}): {s}", self.format)
            }
            other => other,
        })
    }
}

/// A message the driver sends; [`EncodeMessage`] frames it.
pub(crate) trait FrontendMessage: Sized {
    const FORMAT: FrontendMessageFormat;

    /// How many bytes the body takes, to reserve before [`Self::encode_body()`] writes it.
    fn body_size_hint(&self) -> Saturating<usize>;

    /// Writes the body alone: [`EncodeMessage`] writes the format byte and the length.
    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), Error>;

    #[inline(always)]
    #[cfg_attr(not(test), allow(dead_code))]
    fn encode_msg(self, buf: &mut Vec<u8>) -> Result<(), Error> {
        EncodeMessage(self).encode(buf)
    }
}

/// A message the server sends.
pub(crate) trait BackendMessage: Sized {
    const FORMAT: BackendMessageFormat;

    /// Reads the body: the format byte and the length have already been taken off.
    fn decode_body(buf: Bytes) -> Result<Self, Error>;
}

/// A [`FrontendMessage`] framed for the wire: format byte, length, body.
pub struct EncodeMessage<F>(pub F);

impl<F: FrontendMessage> ProtocolEncode<'_, ()> for EncodeMessage<F> {
    fn encode_with(&self, buf: &mut Vec<u8>, _context: ()) -> Result<(), Error> {
        // the body, the format byte and the `i32` length
        let size = self.0.body_size_hint() + Saturating(5);

        // a hint can be absurd; fail on it instead of aborting on an allocation
        buf.try_reserve(size.0).map_err(|e| {
            err_protocol!(
                "Postgres protocol: error allocating {} bytes for encoding message {:?}: {e}",
                size.0,
                F::FORMAT,
            )
        })?;

        buf.push(F::FORMAT as u8);
        buf.put_length_prefixed(|buf| self.0.encode_body(buf))
    }
}

/// Declares messages that are only their format byte, with no body: each struct's name is also
/// its format's.
macro_rules! bodiless {
    (@frontend $name:ident) => {
        impl FrontendMessage for $name {
            const FORMAT: FrontendMessageFormat = FrontendMessageFormat::$name;

            fn body_size_hint(&self) -> Saturating<usize> {
                Saturating(0)
            }

            fn encode_body(&self, _buf: &mut Vec<u8>) -> Result<(), Error> {
                Ok(())
            }
        }
    };
    (@backend $name:ident) => {
        impl BackendMessage for $name {
            const FORMAT: BackendMessageFormat = BackendMessageFormat::$name;

            fn decode_body(_buf: Bytes) -> Result<Self, Error> {
                Ok(Self)
            }
        }
    };
    ($($(#[$doc:meta])* $direction:ident $name:ident;)+) => {$(
        $(#[$doc])*
        #[derive(Debug)]
        pub struct $name;

        bodiless!(@$direction $name);
    )+};
}

bodiless! {
    /// Ends a batch of extended-query messages; the server answers with `ReadyForQuery`.
    frontend Sync;
    /// Tells the server the client is closing the connection.
    frontend Terminate;
    /// The server's acknowledgement of a `Parse`.
    backend ParseComplete;
}

#[cfg(test)]
#[path = "../../tests/unit/message_fuzz_tests.rs"]
mod fuzz_tests;
