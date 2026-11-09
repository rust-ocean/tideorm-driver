use byteorder::{BigEndian, ByteOrder};
use driver_core::bytes::Bytes;
use std::ops::Range;

use crate::error::Error;
use crate::message::{BackendMessage, BackendMessageFormat};

/// A row of data from the database.
#[derive(Debug)]
pub struct DataRow {
    pub(crate) storage: Bytes,

    /// Ranges into the stored row data.
    /// This uses `u32` instead of usize to reduce the size of this type. Values cannot be larger
    /// than `i32` in postgres.
    pub(crate) values: Vec<Option<Range<u32>>>,
}

impl DataRow {
    #[inline]
    pub(crate) fn get(&self, index: usize) -> Option<&'_ [u8]> {
        self.values[index]
            .as_ref()
            .map(|col| &self.storage[(col.start as usize)..(col.end as usize)])
    }
}

impl BackendMessage for DataRow {
    const FORMAT: BackendMessageFormat = BackendMessageFormat::DataRow;

    fn decode_body(buf: Bytes) -> Result<Self, Error> {
        if buf.len() < 2 {
            return Err(err_protocol!(
                "expected at least 2 bytes, got {}",
                buf.len()
            ));
        }

        let cnt = BigEndian::read_u16(&buf) as usize;

        let mut values = Vec::with_capacity(cnt);
        let mut offset: u32 = 2;

        for _ in 0..cnt {
            let value_start = offset
                .checked_add(4)
                .ok_or_else(|| err_protocol!("next value start out of range (offset: {offset})"))?;

            // widen both to a larger type for a safe comparison
            if (buf.len() as u64) < (value_start as u64) {
                return Err(err_protocol!(
                    "expected 4 bytes at offset {offset}, got {}",
                    (value_start as u64) - (buf.len() as u64)
                ));
            }

            // Length of the column value, in bytes (this count does not include itself).
            // Can be zero. As a special case, -1 indicates a NULL column value.
            // No value bytes follow in the NULL case.
            //
            // we know `offset` is within range of `buf.len()` from the above check
            #[allow(clippy::cast_possible_truncation)]
            let length = BigEndian::read_i32(&buf[(offset as usize)..]);

            if let Ok(length) = u32::try_from(length) {
                let value_end = value_start.checked_add(length).ok_or_else(|| {
                    err_protocol!("value_start + length out of range ({offset} + {length})")
                })?;

                if (buf.len() as u64) < (value_end as u64) {
                    return Err(err_protocol!(
                        "value of {length} bytes at offset {value_start} ends past the {}-byte row",
                        buf.len()
                    ));
                }

                values.push(Some(value_start..value_end));
                offset = value_end;
            } else if length == -1 {
                values.push(None);
                // `value_start` is actually the next value now.
                offset = value_start;
            } else {
                return Err(err_protocol!(
                    "invalid value length {length} at offset {offset}"
                ));
            }
        }

        Ok(Self {
            storage: buf,
            values,
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_data_row_tests.rs"]
mod tests;
