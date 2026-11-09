use bytes::{Buf, Bytes};

use crate::error::Error;
use crate::io::{BufExt, ProtocolDecode};
use crate::protocol::Capabilities;

// https://dev.mysql.com/doc/dev/mysql-server/8.0.12/page_protocol_basic_err_packet.html
// https://mariadb.com/kb/en/err_packet/

/// Indicates that an error occurred.
#[derive(Debug)]
pub struct ErrPacket {
    pub error_code: u16,
    pub sql_state: Option<String>,
    pub error_message: String,
}

impl ProtocolDecode<'_, Capabilities> for ErrPacket {
    fn decode_with(mut buf: Bytes, capabilities: Capabilities) -> Result<Self, Error> {
        if buf.len() < 3 {
            return Err(err_protocol!(
                "ERR_Packet too short: expected at least 3 bytes, got {}",
                buf.len()
            ));
        }

        let header = buf.get_u8();
        if header != 0xff {
            return Err(err_protocol!(
                "expected 0xff (ERR_Packet) but found 0x{:x}",
                header
            ));
        }

        let error_code = buf.get_u16_le();
        let mut sql_state = None;

        if capabilities.contains(Capabilities::PROTOCOL_41) {
            // If the next byte is '#' then we have a SQL STATE
            if buf.starts_with(b"#") {
                buf.advance(1);
                sql_state = Some(buf.get_str(5)?);
            }
        }

        let error_message = buf.get_str(buf.len())?;

        Ok(Self {
            error_code,
            sql_state,
            error_message,
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/protocol_response_err_tests.rs"]
mod tests;
