use byteorder::{BigEndian, ByteOrder};
use driver_core::bytes::Bytes;

use crate::error::Error;
use crate::message::{BackendMessage, BackendMessageFormat};

/// Contains cancellation key data. The frontend must save these values if it
/// wishes to be able to issue `CancelRequest` messages later.
#[derive(Debug)]
pub struct BackendKeyData {
    /// The process ID of this database.
    pub process_id: u32,

    /// The secret key of this database.
    pub secret_key: u32,
}

impl BackendMessage for BackendKeyData {
    const FORMAT: BackendMessageFormat = BackendMessageFormat::BackendKeyData;

    fn decode_body(buf: Bytes) -> Result<Self, Error> {
        if buf.len() < 8 {
            return Err(err_protocol!(
                "expected at least 8 bytes, got {}",
                buf.len()
            ));
        }

        let process_id = BigEndian::read_u32(&buf);
        let secret_key = BigEndian::read_u32(&buf[4..]);

        Ok(Self {
            process_id,
            secret_key,
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_backend_key_data_tests.rs"]
mod tests;
