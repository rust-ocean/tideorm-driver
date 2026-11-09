use crate::io::BufMutExt;
use crate::message::{FrontendMessage, FrontendMessageFormat};
use driver_core::Error;
use std::num::Saturating;

#[derive(Debug)]
pub struct Query<'a>(pub &'a str);

impl FrontendMessage for Query<'_> {
    const FORMAT: FrontendMessageFormat = FrontendMessageFormat::Query;

    fn body_size_hint(&self) -> Saturating<usize> {
        let mut size = Saturating(0);

        size += self.0.len();
        size += 1; // NUL terminator

        size
    }

    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        buf.put_str_nul(self.0);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_query_tests.rs"]
mod tests;
