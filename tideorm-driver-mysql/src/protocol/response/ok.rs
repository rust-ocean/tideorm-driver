use bytes::{Buf, Bytes};

use crate::error::Error;
use crate::io::MySqlBufExt;
use crate::io::ProtocolDecode;
use crate::protocol::response::Status;

/// Indicates successful completion of a previous command sent by the client.
#[derive(Debug)]
pub struct OkPacket {
    pub affected_rows: u64,
    pub last_insert_id: u64,
    pub status: Status,
    pub warnings: u16,
}

impl ProtocolDecode<'_> for OkPacket {
    fn decode_with(mut buf: Bytes, _: ()) -> Result<Self, Error> {
        let header = buf.get_u8();
        if header != 0 && header != 0xfe {
            return Err(err_protocol!(
                "expected 0x00 or 0xfe (OK_Packet) but found 0x{:02x}",
                header
            ));
        }

        let affected_rows = buf.get_uint_lenenc()?;
        let last_insert_id = buf.get_uint_lenenc()?;

        if buf.remaining() < 4 {
            return Err(err_protocol!(
                "OK_Packet too short: expected at least 4 more bytes for status+warnings, got {}",
                buf.remaining()
            ));
        }

        let status = Status::from_bits_truncate(buf.get_u16_le());
        let warnings = buf.get_u16_le();

        Ok(Self {
            affected_rows,
            last_insert_id,
            status,
            warnings,
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/protocol_response_ok_tests.rs"]
mod tests;
