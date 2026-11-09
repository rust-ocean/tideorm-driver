use bytes::buf::Chain;
use bytes::{Buf, Bytes};
use std::cmp;

use crate::error::Error;
use crate::io::{BufExt, ProtocolDecode};
use crate::protocol::auth::AuthPlugin;
use crate::protocol::response::Status;
use crate::protocol::Capabilities;

// https://dev.mysql.com/doc/internals/en/connection-phase-packets.html#packet-Protocol::Handshake
// https://mariadb.com/kb/en/connection/#initial-handshake-packet

#[derive(Debug)]
pub(crate) struct Handshake {
    #[allow(unused)]
    pub(crate) protocol_version: u8,
    pub(crate) server_version: String,
    #[allow(unused)]
    pub(crate) connection_id: u32,
    pub(crate) server_capabilities: Capabilities,
    #[allow(unused)]
    pub(crate) server_default_collation: u8,
    #[allow(unused)]
    pub(crate) status: Status,
    pub(crate) auth_plugin: Option<AuthPlugin>,
    pub(crate) auth_plugin_data: Chain<Bytes, Bytes>,
}

impl ProtocolDecode<'_> for Handshake {
    fn decode_with(mut buf: Bytes, _: ()) -> Result<Self, Error> {
        // int<1>
        let protocol_version = *buf
            .first()
            .ok_or_else(|| err_protocol!("empty handshake"))?;
        buf.advance(1);
        let server_version = buf.get_str_nul()?; // string<NUL>

        // the fixed fields, through the 4 reserved or capability bytes
        if buf.remaining() < 31 {
            return Err(err_protocol!(
                "handshake too short: expected 31 more bytes, got {}",
                buf.remaining()
            ));
        }

        let connection_id = buf.get_u32_le(); // int<4>
        let auth_plugin_data_1 = buf.get_bytes(8)?; // string<8>

        buf.advance(1); // reserved: string<1>

        let capabilities_1 = buf.get_u16_le(); // int<2>
        let mut capabilities = Capabilities::from_bits_truncate(capabilities_1.into());

        let collation = buf.get_u8(); // int<1>
        let status = Status::from_bits_truncate(buf.get_u16_le());

        let capabilities_2 = buf.get_u16_le(); // int<2>
        capabilities |= Capabilities::from_bits_truncate(((capabilities_2 as u32) << 16).into());

        let auth_plugin_data_len = if capabilities.contains(Capabilities::PLUGIN_AUTH) {
            buf.get_u8()
        } else {
            buf.advance(1); // int<1>
            0
        };

        buf.advance(6); // reserved: string<6>

        if capabilities.contains(Capabilities::MYSQL) {
            buf.advance(4); // reserved: string<4>
        } else {
            let capabilities_3 = buf.get_u32_le(); // int<4>
            capabilities |= Capabilities::from_bits_truncate((capabilities_3 as u64) << 32);
        }

        let auth_plugin_data_2 = if capabilities.contains(Capabilities::SECURE_CONNECTION) {
            let len = usize::from(cmp::max(auth_plugin_data_len.saturating_sub(9), 12));
            // the data, then its NUL terminator
            buf.get_bytes(len + 1)?.slice(..len)
        } else {
            Bytes::new()
        };

        let auth_plugin = if capabilities.contains(Capabilities::PLUGIN_AUTH) {
            Some(buf.get_str_nul()?.parse()?)
        } else {
            None
        };

        Ok(Self {
            protocol_version,
            server_version,
            connection_id,
            server_default_collation: collation,
            status,
            server_capabilities: capabilities,
            auth_plugin,
            auth_plugin_data: auth_plugin_data_1.chain(auth_plugin_data_2),
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/protocol_connect_handshake_tests.rs"]
mod tests;
