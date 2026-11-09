//! The client's answer to the server's greeting, and the shorter request to
//! switch to TLS first, which is the answer's first 32 bytes.
//!
//! <https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase_packets_protocol_handshake_response.html>
//! <https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase_packets_protocol_ssl_request.html>
//! <https://mariadb.com/kb/en/connection/#client-handshake-response>

use crate::io::MySqlBufMutExt;
use crate::io::{BufMutExt, ProtocolEncode};
use crate::protocol::auth::AuthPlugin;
use crate::protocol::Capabilities;

/// Asks the server to start TLS before the client authenticates.
#[derive(Debug)]
pub struct SslRequest {
    pub max_packet_size: u32,
    /// The connection's collation ID; only IDs under 256 fit.
    pub charset: u8,
}

impl ProtocolEncode<'_, Capabilities> for SslRequest {
    fn encode_with(&self, buf: &mut Vec<u8>, context: Capabilities) -> Result<(), crate::Error> {
        let bits = context.bits().to_le_bytes();
        let (low, high) = bits.split_at(4);

        buf.extend_from_slice(low);
        buf.extend_from_slice(&self.max_packet_size.to_le_bytes());
        buf.push(self.charset);
        buf.extend_from_slice(&[0; 19]);
        // MySQL reserves the last four bytes; MariaDB reads its extended
        // capabilities there.
        if context.contains(Capabilities::MYSQL) {
            buf.extend_from_slice(&[0; 4]);
        } else {
            buf.extend_from_slice(high);
        }

        Ok(())
    }
}

/// Logs in: who the client is, its proof of the password and the database to
/// start in.
#[derive(Debug)]
pub struct HandshakeResponse<'a> {
    pub database: Option<&'a str>,

    /// The largest command packet the client will send.
    pub max_packet_size: u32,

    /// The connection's collation ID; only IDs under 256 fit.
    pub charset: u8,

    pub username: &'a str,

    /// The plugin `auth_response` was made for.
    pub auth_plugin: Option<AuthPlugin>,

    /// The plugin's answer to the server's challenge.
    pub auth_response: Option<&'a [u8]>,
}

impl ProtocolEncode<'_, Capabilities> for HandshakeResponse<'_> {
    fn encode_with(
        &self,
        buf: &mut Vec<u8>,
        mut context: Capabilities,
    ) -> Result<(), crate::Error> {
        // Without a plugin there is no plugin name to send.
        if self.auth_plugin.is_none() {
            context.remove(Capabilities::PLUGIN_AUTH);
        }

        SslRequest {
            max_packet_size: self.max_packet_size,
            charset: self.charset,
        }
        .encode_with(buf, context)?;

        buf.put_str_nul(self.username);

        let auth_response = self.auth_response.unwrap_or_default();
        if context.contains(Capabilities::PLUGIN_AUTH_LENENC_DATA) {
            buf.put_bytes_lenenc(auth_response);
        } else if context.contains(Capabilities::SECURE_CONNECTION) {
            let len = u8::try_from(auth_response.len()).map_err(|_| {
                err_protocol!("auth_response.len() too long: {}", auth_response.len())
            })?;
            buf.push(len);
            buf.extend_from_slice(auth_response);
        } else {
            buf.push(0);
        }

        // An absent database or plugin is written as an empty name.
        if context.contains(Capabilities::CONNECT_WITH_DB) {
            buf.put_str_nul(self.database.unwrap_or_default());
        }
        if context.contains(Capabilities::PLUGIN_AUTH) {
            buf.put_str_nul(self.auth_plugin.map_or("", AuthPlugin::name));
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/protocol_connect_handshake_response_tests.rs"]
mod tests;
