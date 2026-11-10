//! Byte-level pieces the wire protocols share: reading and writing fields of a frame, and the
//! traits a protocol message implements to be read from or written to a connection.

mod buf;
mod read_buf;

use bytes::{BufMut, Bytes};

use crate::error::Error;

pub use buf::BufExt;
pub use read_buf::ReadBuf;

// `read_from` sources take the reading traits of the runtime the streams come from.
#[cfg(not(feature = "_rt-tokio"))]
pub use futures_util::io::{AsyncRead, AsyncReadExt};
#[cfg(feature = "_rt-tokio")]
pub use tokio::io::{AsyncRead, AsyncReadExt};

/// Writes fields of an outgoing frame.
pub trait BufMutExt: BufMut {
    /// Appends `s` and a terminating NUL, the protocols' C-string form.
    fn put_str_nul(&mut self, s: &str);
}

impl BufMutExt for Vec<u8> {
    fn put_str_nul(&mut self, s: &str) {
        self.extend_from_slice(s.as_bytes());
        self.push(0);
    }
}

/// A message read from a connection, built from the bytes of one received frame.
///
/// `Context` is what decoding needs besides the bytes, such as the negotiated capabilities or
/// the columns a row is read against. A message that needs nothing takes `()` and can be read
/// with [`decode`](Self::decode).
pub trait ProtocolDecode<'de, Context = ()>: Sized {
    /// Decodes a message that takes no context.
    fn decode(buf: Bytes) -> Result<Self, Error>
    where
        Self: ProtocolDecode<'de, ()>,
    {
        <Self as ProtocolDecode<'de>>::decode_with(buf, ())
    }

    /// Decodes a message from `buf`, given `context`.
    fn decode_with(buf: Bytes, context: Context) -> Result<Self, Error>;
}

/// The frame's bytes as received.
impl ProtocolDecode<'_> for Bytes {
    fn decode_with(buf: Bytes, (): ()) -> Result<Self, Error> {
        Ok(buf)
    }
}

/// A frame whose contents nobody reads.
impl ProtocolDecode<'_> for () {
    fn decode_with(_: Bytes, (): ()) -> Result<Self, Error> {
        Ok(())
    }
}

/// A message written to a connection, appended to the outgoing buffer.
///
/// As with [`ProtocolDecode`], `Context` is what encoding needs besides the message.
pub trait ProtocolEncode<'en, Context = ()> {
    /// Encodes a message that takes no context.
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), Error>
    where
        Self: ProtocolEncode<'en, ()>,
    {
        <Self as ProtocolEncode<'en>>::encode_with(self, buf, ())
    }

    /// Appends the message to `buf`, given `context`.
    fn encode_with(&self, buf: &mut Vec<u8>, context: Context) -> Result<(), Error>;
}

/// Bytes already in wire form, written unchanged whatever the context.
impl<C> ProtocolEncode<'_, C> for &'_ [u8] {
    fn encode_with(&self, buf: &mut Vec<u8>, _: C) -> Result<(), Error> {
        buf.extend_from_slice(self);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/io_tests.rs"]
mod tests;
