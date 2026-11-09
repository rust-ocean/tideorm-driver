use crate::io::BufMutExt;
use crate::message::{FrontendMessage, FrontendMessageFormat};
use driver_core::Error;
use std::num::Saturating;

/// `SASLInitialResponse`: the SCRAM mechanism the client chose and its first message.
pub struct SaslInitialResponse<'a> {
    pub response: &'a str,
    /// Whether the exchange binds the TLS channel (`SCRAM-SHA-256-PLUS`).
    pub plus: bool,
}

impl SaslInitialResponse<'_> {
    fn mechanism(&self) -> &'static str {
        if self.plus {
            "SCRAM-SHA-256-PLUS"
        } else {
            "SCRAM-SHA-256"
        }
    }
}

impl FrontendMessage for SaslInitialResponse<'_> {
    const FORMAT: FrontendMessageFormat = FrontendMessageFormat::PasswordPolymorphic;

    fn body_size_hint(&self) -> Saturating<usize> {
        // the mechanism and its NUL, the response's `i32` length, the response
        Saturating(self.mechanism().len()) + Saturating(1 + 4) + Saturating(self.response.len())
    }

    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        buf.put_str_nul(self.mechanism());

        let len = i32::try_from(self.response.len()).map_err(|_| {
            err_protocol!(
                "SASL Initial Response length too long for protocol: {}",
                self.response.len()
            )
        })?;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(self.response.as_bytes());

        Ok(())
    }
}

/// `SASLResponse`: each later SCRAM message from the client, sent as it is.
pub struct SaslResponse<'a>(pub &'a str);

impl FrontendMessage for SaslResponse<'_> {
    const FORMAT: FrontendMessageFormat = FrontendMessageFormat::PasswordPolymorphic;

    fn body_size_hint(&self) -> Saturating<usize> {
        Saturating(self.0.len())
    }

    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        buf.extend_from_slice(self.0.as_bytes());
        Ok(())
    }
}
