use bytes::{Buf, Bytes};

use crate::error::Error;
use crate::io::ProtocolDecode;
use crate::protocol::response::Status;
use crate::protocol::Capabilities;

/// Marks the end of a result set, returning status and warnings.
///
/// # Note
///
/// The EOF packet is deprecated as of MySQL 5.7.5. The driver only uses this packet for
/// older MySQL versions.
#[derive(Debug)]
pub struct EofPacket {
    #[allow(dead_code)]
    pub warnings: u16,
    pub status: Status,
}

impl ProtocolDecode<'_, Capabilities> for EofPacket {
    fn decode_with(mut buf: Bytes, _: Capabilities) -> Result<Self, Error> {
        if buf.len() < 5 {
            return Err(err_protocol!(
                "EOF_Packet too short: expected 5 bytes, got {}",
                buf.len()
            ));
        }

        let header = buf.get_u8();
        if header != 0xfe {
            return Err(err_protocol!(
                "expected 0xfe (EOF_Packet) but found 0x{:x}",
                header
            ));
        }

        let warnings = buf.get_u16_le();
        let status = Status::from_bits_truncate(buf.get_u16_le());

        Ok(Self { status, warnings })
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/protocol_response_eof_tests.rs"]
mod tests;
