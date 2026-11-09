use crate::io::ProtocolEncode;

pub struct SslRequest;

impl SslRequest {
    // https://www.postgresql.org/docs/current/protocol-message-formats.html#PROTOCOL-MESSAGE-FORMATS-SSLREQUEST
    pub const BYTES: &'static [u8] = b"\x00\x00\x00\x08\x04\xd2\x16\x2f";
}

// Cannot impl FrontendMessage because it does not have a format code
impl ProtocolEncode<'_> for SslRequest {
    #[inline(always)]
    fn encode_with(&self, buf: &mut Vec<u8>, _context: ()) -> Result<(), crate::Error> {
        buf.extend_from_slice(Self::BYTES);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_ssl_request_tests.rs"]
mod tests;
