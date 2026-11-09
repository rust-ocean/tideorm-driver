use atoi::atoi;
use driver_core::bytes::Bytes;
use memchr::memrchr;

use crate::error::Error;
use crate::message::{BackendMessage, BackendMessageFormat};

#[derive(Debug)]
pub struct CommandComplete {
    /// The command tag. This is usually a single word that identifies which SQL command
    /// was completed.
    tag: Bytes,
}

impl BackendMessage for CommandComplete {
    const FORMAT: BackendMessageFormat = BackendMessageFormat::CommandComplete;

    fn decode_body(bytes: Bytes) -> Result<Self, Error> {
        Ok(CommandComplete { tag: bytes })
    }
}

impl CommandComplete {
    /// Returns the number of rows affected.
    /// If the command does not return rows (e.g., "CREATE TABLE"), returns 0.
    pub fn rows_affected(&self) -> u64 {
        // Look backwards for the first SPACE
        memrchr(b' ', &self.tag)
            // This is either a word or the number of rows affected
            .and_then(|i| atoi(&self.tag[(i + 1)..]))
            .unwrap_or(0)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_command_complete_tests.rs"]
mod tests;
