use std::num::Saturating;

use driver_core::Error;

use crate::io::{PgBufMutExt, PortalId};
use crate::message::{FrontendMessage, FrontendMessageFormat};

pub struct Execute {
    /// The id of the portal to execute.
    pub portal: PortalId,

    /// Maximum number of rows to return, if portal contains a query
    /// that returns rows (ignored otherwise). Zero denotes “no limit”.
    pub limit: u32,
}

impl FrontendMessage for Execute {
    const FORMAT: FrontendMessageFormat = FrontendMessageFormat::Execute;

    fn body_size_hint(&self) -> Saturating<usize> {
        let mut size = Saturating(0);

        size += self.portal.name_len();
        size += 2; // limit

        size
    }

    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        buf.put_portal_name(self.portal);
        buf.extend(&self.limit.to_be_bytes());

        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_execute_tests.rs"]
mod tests;
