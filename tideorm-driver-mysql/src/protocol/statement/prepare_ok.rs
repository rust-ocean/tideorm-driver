use bytes::{Buf, Bytes};

use crate::error::Error;
use crate::io::ProtocolDecode;
use crate::protocol::Capabilities;

/// The first packet of a successful prepare. The definitions of its
/// parameters and then of its result columns follow it.
///
/// <https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_com_stmt_prepare.html#sect_protocol_com_stmt_prepare_response_ok>
#[derive(Debug)]
pub(crate) struct PrepareOk {
    pub(crate) statement_id: u32,
    pub(crate) columns: u16,
    pub(crate) params: u16,
}

impl ProtocolDecode<'_, Capabilities> for PrepareOk {
    fn decode_with(buf: Bytes, _: Capabilities) -> Result<Self, Error> {
        // A filler byte and the warning count end the 12 bytes read here;
        // newer servers may add a metadata flag after them.
        let mut fields = buf
            .first_chunk::<12>()
            .ok_or_else(|| {
                err_protocol!("PrepareOk expected 12 bytes but got {} bytes", buf.len())
            })?
            .as_slice();

        let status = fields.get_u8();
        if status != 0x00 {
            return Err(err_protocol!(
                "expected 0x00 (COM_STMT_PREPARE_OK) but found 0x{:02x}",
                status
            ));
        }

        Ok(Self {
            statement_id: fields.get_u32_le(),
            columns: fields.get_u16_le(),
            params: fields.get_u16_le(),
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/protocol_statement_prepare_ok_tests.rs"]
mod tests;
