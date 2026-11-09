use crate::io::BufMutExt;
use crate::io::{PgBufMutExt, StatementId};
use crate::message::{FrontendMessage, FrontendMessageFormat};
use crate::types::Oid;
use driver_core::Error;
use std::num::Saturating;

#[derive(Debug)]
pub struct Parse<'a> {
    /// The ID of the destination prepared statement.
    pub statement: StatementId,

    /// The query string to be parsed.
    pub query: &'a str,

    /// The parameter data types specified (could be zero). Note that this is not an
    /// indication of the number of parameters that might appear in the query string,
    /// only the number that the frontend wants to pre-specify types for.
    pub param_types: &'a [Oid],
}

impl FrontendMessage for Parse<'_> {
    const FORMAT: FrontendMessageFormat = FrontendMessageFormat::Parse;

    fn body_size_hint(&self) -> Saturating<usize> {
        let mut size = Saturating(0);

        size += self.statement.name_len();

        size += self.query.len();
        size += 1; // NUL terminator

        size += 2; // param_types_len

        // `param_types`
        size += self.param_types.len().saturating_mul(4);

        size
    }

    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        buf.put_statement_name(self.statement);

        buf.put_str_nul(self.query);

        // Note: actually interpreted as unsigned.
        let param_types_len = u16::try_from(self.param_types.len()).map_err(|_| {
            err_protocol!(
                "param_types.len() too large for binary protocol: {}",
                self.param_types.len()
            )
        })?;

        buf.extend(param_types_len.to_be_bytes());

        for &oid in self.param_types {
            buf.extend(oid.0.to_be_bytes());
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_parse_tests.rs"]
mod tests;
