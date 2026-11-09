use driver_core::bytes::Bytes;

use crate::error::Error;
use crate::io::BufExt;
use crate::message::{BackendMessage, BackendMessageFormat};

#[derive(Debug)]
pub struct ParameterStatus {
    pub name: String,
    pub value: String,
}

impl BackendMessage for ParameterStatus {
    const FORMAT: BackendMessageFormat = BackendMessageFormat::ParameterStatus;

    fn decode_body(mut buf: Bytes) -> Result<Self, Error> {
        let name = buf.get_str_nul()?;
        let value = buf.get_str_nul()?;

        Ok(Self { name, value })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_parameter_status_tests.rs"]
mod tests;
