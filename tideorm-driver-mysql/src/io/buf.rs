use bytes::{Buf, Bytes};

use crate::error::Error;

pub trait MySqlBufExt: Buf {
    // Read a length-encoded integer.
    // 0xfb (a text row's NULL) and 0xff (an ERR packet) begin no integer: callers that
    // can meet them check for them first, and here they are protocol errors.
    // <https://dev.mysql.com/doc/internals/en/integer.html#packet-Protocol::LengthEncodedInteger>
    fn get_uint_lenenc(&mut self) -> Result<u64, Error>;

    // Read a length-encoded byte sequence.
    fn get_bytes_lenenc(&mut self) -> Result<Bytes, Error>;
}

impl MySqlBufExt for Bytes {
    fn get_uint_lenenc(&mut self) -> Result<u64, Error> {
        if self.remaining() < 1 {
            return Err(err_protocol!("lenenc int: no bytes remaining"));
        }

        match self.get_u8() {
            0xfc => {
                if self.remaining() < 2 {
                    return Err(err_protocol!(
                        "lenenc int: need 2 more bytes, have {}",
                        self.remaining()
                    ));
                }
                Ok(u64::from(self.get_u16_le()))
            }
            0xfd => {
                if self.remaining() < 3 {
                    return Err(err_protocol!(
                        "lenenc int: need 3 more bytes, have {}",
                        self.remaining()
                    ));
                }
                Ok(self.get_uint_le(3))
            }
            0xfe => {
                if self.remaining() < 8 {
                    return Err(err_protocol!(
                        "lenenc int: need 8 more bytes, have {}",
                        self.remaining()
                    ));
                }
                Ok(self.get_u64_le())
            }

            v @ (0xfb | 0xff) => Err(err_protocol!("lenenc int: unexpected marker 0x{v:02x}")),

            v => Ok(u64::from(v)),
        }
    }

    fn get_bytes_lenenc(&mut self) -> Result<Bytes, Error> {
        let size = self.get_uint_lenenc()?;
        let size = usize::try_from(size)
            .ok()
            .filter(|&size| size <= self.remaining())
            .ok_or_else(|| {
                err_protocol!("lenenc bytes: need {size} bytes, have {}", self.remaining())
            })?;

        Ok(self.split_to(size))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/io_buf_tests.rs"]
mod tests;
