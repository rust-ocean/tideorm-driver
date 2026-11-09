use driver_core::bytes::{Buf, Bytes};

use crate::error::Error;
use crate::io::BufExt;
use crate::message::{BackendMessage, BackendMessageFormat};

#[derive(Debug, Clone)]
pub struct Notification {
    pub(crate) process_id: u32,
    pub(crate) channel: Bytes,
    pub(crate) payload: Bytes,
}

impl BackendMessage for Notification {
    const FORMAT: BackendMessageFormat = BackendMessageFormat::NotificationResponse;

    fn decode_body(mut buf: Bytes) -> Result<Self, Error> {
        if buf.len() < 4 {
            return Err(err_protocol!(
                "expected at least 4 bytes, got {}",
                buf.len()
            ));
        }

        let process_id = buf.get_u32();
        let channel = buf.get_bytes_nul()?;
        let payload = buf.get_bytes_nul()?;

        Ok(Self {
            process_id,
            channel,
            payload,
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_notification_tests.rs"]
mod tests;
