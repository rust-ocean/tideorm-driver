use driver_core::bytes::{Buf, Bytes};
use smallvec::SmallVec;

use crate::error::Error;
use crate::message::{BackendMessage, BackendMessageFormat};
use crate::types::Oid;

#[derive(Debug)]
pub struct ParameterDescription {
    pub types: SmallVec<[Oid; 6]>,
}

impl BackendMessage for ParameterDescription {
    const FORMAT: BackendMessageFormat = BackendMessageFormat::ParameterDescription;

    fn decode_body(mut buf: Bytes) -> Result<Self, Error> {
        if buf.len() < 2 {
            return Err(err_protocol!(
                "expected at least 2 bytes, got {}",
                buf.len()
            ));
        }

        // Note: this is correct, max parameters is 65535, not 32767.
        let cnt = buf.get_u16();
        if buf.len() < 4 * usize::from(cnt) {
            return Err(err_protocol!(
                "expected {cnt} parameter types, got {} bytes",
                buf.len()
            ));
        }

        let mut types = SmallVec::with_capacity(cnt as usize);

        for _ in 0..cnt {
            types.push(Oid(buf.get_u32()));
        }

        Ok(Self { types })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_parameter_description_tests.rs"]
mod tests;
