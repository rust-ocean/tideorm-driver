use driver_core::bytes::Bytes;

use crate::error::Error;
use crate::message::{BackendMessage, BackendMessageFormat};

#[derive(Debug)]
#[repr(u8)]
pub enum TransactionStatus {
    /// Not in a transaction block.
    Idle = b'I',

    /// In a transaction block.
    Transaction = b'T',

    /// In a _failed_ transaction block. Queries will be rejected until block is ended.
    Error = b'E',
}

#[derive(Debug)]
pub struct ReadyForQuery {
    pub transaction_status: TransactionStatus,
}

impl BackendMessage for ReadyForQuery {
    const FORMAT: BackendMessageFormat = BackendMessageFormat::ReadyForQuery;

    fn decode_body(buf: Bytes) -> Result<Self, Error> {
        let status = match buf.first() {
            Some(b'I') => TransactionStatus::Idle,
            Some(b'T') => TransactionStatus::Transaction,
            Some(b'E') => TransactionStatus::Error,

            status => {
                return Err(err_protocol!(
                    "unknown transaction status: {:?}",
                    status.map(|&status| status as char)
                ));
            }
        };

        Ok(Self {
            transaction_status: status,
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_ready_for_query_tests.rs"]
mod tests;
