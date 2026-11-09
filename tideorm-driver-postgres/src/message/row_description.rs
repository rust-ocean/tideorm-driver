use driver_core::bytes::{Buf, Bytes};

use crate::error::Error;
use crate::io::BufExt;
use crate::message::{BackendMessage, BackendMessageFormat};
use crate::types::Oid;

/// The columns a statement returns: sent before its rows, and in answer to a `Describe`.
#[derive(Debug)]
pub struct RowDescription {
    pub fields: Vec<Field>,
}

/// One column of a [`RowDescription`].
#[derive(Debug)]
pub struct Field {
    pub name: String,

    /// The table the column is read from, when it is a plain table column.
    pub relation_id: Option<Oid>,

    /// The column's attribute number in that table.
    pub relation_attribute_no: Option<i16>,

    pub data_type_id: Oid,
}

impl BackendMessage for RowDescription {
    const FORMAT: BackendMessageFormat = BackendMessageFormat::RowDescription;

    fn decode_body(mut buf: Bytes) -> Result<Self, Error> {
        if buf.len() < 2 {
            return Err(err_protocol!(
                "expected at least 2 bytes, got {}",
                buf.len()
            ));
        }

        let count = buf.get_u16();
        let mut fields = Vec::with_capacity(count.into());

        for _ in 0..count {
            let name = buf.get_str_nul()?;

            // table OID, attribute number, type OID, type size, type modifier, format code
            if buf.len() < 18 {
                return Err(err_protocol!(
                    "expected at least 18 bytes after field name {name:?}, got {}",
                    buf.len()
                ));
            }

            // Both are zero for a column that is not read straight from a table.
            let relation_id = Some(buf.get_u32()).filter(|&id| id != 0).map(Oid);
            let relation_attribute_no = Some(buf.get_i16()).filter(|&no| no != 0);
            let data_type_id = Oid(buf.get_u32());
            // The type's size and modifier, and the column's format code, are not used.
            buf.advance(2 + 4 + 2);

            fields.push(Field {
                name,
                relation_id,
                relation_attribute_no,
                data_type_id,
            });
        }

        Ok(Self { fields })
    }
}
