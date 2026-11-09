use bytes::BufMut;

pub trait MySqlBufMutExt: BufMut {
    fn put_uint_lenenc(&mut self, v: u64);

    fn put_str_lenenc(&mut self, v: &str);

    fn put_bytes_lenenc(&mut self, v: &[u8]);
}

impl MySqlBufMutExt for Vec<u8> {
    fn put_uint_lenenc(&mut self, v: u64) {
        // https://dev.mysql.com/doc/internals/en/integer.html
        // https://mariadb.com/kb/en/library/protocol-data-types/#length-encoded-integers

        let encoded_le = v.to_le_bytes();

        match v {
            0..=250 => self.push(encoded_le[0]),
            251..=0xFF_FF => {
                self.push(0xfc);
                self.extend_from_slice(&encoded_le[..2]);
            }
            0x1_00_00..=0xFF_FF_FF => {
                self.push(0xfd);
                self.extend_from_slice(&encoded_le[..3]);
            }
            _ => {
                self.push(0xfe);
                self.extend_from_slice(&encoded_le);
            }
        }
    }

    fn put_str_lenenc(&mut self, v: &str) {
        self.put_bytes_lenenc(v.as_bytes());
    }

    fn put_bytes_lenenc(&mut self, v: &[u8]) {
        self.put_uint_lenenc(v.len() as u64);
        self.extend(v);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/io_buf_mut_tests.rs"]
mod tests;
