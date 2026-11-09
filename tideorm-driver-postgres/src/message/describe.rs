use crate::io::{PgBufMutExt, StatementId};
use crate::message::{FrontendMessage, FrontendMessageFormat};
use driver_core::Error;
use std::num::Saturating;

const DESCRIBE_STATEMENT: u8 = b'S';

/// Note: will emit both a RowDescription and a ParameterDescription message
#[derive(Debug)]
pub enum Describe {
    Statement(StatementId),
}

impl FrontendMessage for Describe {
    const FORMAT: FrontendMessageFormat = FrontendMessageFormat::Describe;

    fn body_size_hint(&self) -> Saturating<usize> {
        let Describe::Statement(id) = self;
        // `DESCRIBE_STATEMENT`, then the name
        Saturating(1) + id.name_len()
    }

    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        let Describe::Statement(id) = self;
        buf.push(DESCRIBE_STATEMENT);
        buf.put_statement_name(*id);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_describe_tests.rs"]
mod tests;
