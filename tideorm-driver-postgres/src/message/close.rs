use crate::io::{PgBufMutExt, PortalId, StatementId};
use crate::message::{FrontendMessage, FrontendMessageFormat};
use std::num::Saturating;

/// Releases a prepared statement or a portal on the server, which answers with `CloseComplete`.
#[derive(Debug)]
pub enum Close {
    Statement(StatementId),
    Portal(PortalId),
}

impl FrontendMessage for Close {
    const FORMAT: FrontendMessageFormat = FrontendMessageFormat::Close;

    fn body_size_hint(&self) -> Saturating<usize> {
        let name_len = match self {
            Close::Statement(id) => id.name_len(),
            Close::Portal(id) => id.name_len(),
        };
        // the kind byte, then the name
        Saturating(1) + name_len
    }

    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), crate::Error> {
        // the kind byte names which namespace the name is in
        match *self {
            Close::Statement(id) => {
                buf.push(b'S');
                buf.put_statement_name(id);
            }
            Close::Portal(id) => {
                buf.push(b'P');
                buf.put_portal_name(id);
            }
        }

        Ok(())
    }
}
