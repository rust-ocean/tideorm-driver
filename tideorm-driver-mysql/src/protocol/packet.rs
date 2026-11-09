use std::cmp::min;
use std::ops::{Deref, DerefMut};

use bytes::Bytes;

use crate::error::Error;
use crate::io::{ProtocolDecode, ProtocolEncode};
use crate::protocol::response::{EofPacket, OkPacket};
use crate::protocol::Capabilities;

#[derive(Debug)]
pub struct Packet<T>(pub(crate) T);

impl<'en, 'stream, T> ProtocolEncode<'stream, (Capabilities, &'stream mut u8)> for Packet<T>
where
    T: ProtocolEncode<'en, Capabilities>,
{
    fn encode_with(
        &self,
        buf: &mut Vec<u8>,
        (capabilities, sequence_id): (Capabilities, &'stream mut u8),
    ) -> Result<(), Error> {
        let mut next_header = |len: u32| {
            let mut buf = len.to_le_bytes();
            buf[3] = *sequence_id;
            *sequence_id = sequence_id.wrapping_add(1);

            buf
        };

        // reserve space to write the prefixed length
        let offset = buf.len();
        buf.extend(&[0_u8; 4]);

        // encode the payload
        self.0.encode_with(buf, capabilities)?;

        // determine the length of the encoded payload
        // and write to our reserved space
        let len = buf.len() - offset - 4;
        let header = &mut buf[offset..];

        // // `min(.., 0xFF_FF_FF)` cannot overflow
        #[allow(clippy::cast_possible_truncation)]
        header[..4].copy_from_slice(&next_header(min(len, 0xFF_FF_FF) as u32));

        // add more packets if we need to split the data
        if len >= 0xFF_FF_FF {
            let start = offset + 4 + 0xFF_FF_FF;
            let rest = len - 0xFF_FF_FF;

            // a payload that is a multiple of 0xFF_FF_FF ends with an empty part;
            // `size <= 0xFF_FF_FF`
            #[allow(clippy::cast_possible_truncation)]
            let parts: Vec<(usize, [u8; 4])> = (0..=rest / 0xFF_FF_FF)
                .map(|part| {
                    let size = min(rest - part * 0xFF_FF_FF, 0xFF_FF_FF);
                    (size, next_header(size as u32))
                })
                .collect();

            // split in place: each part moves right past the headers before it,
            // the last part first
            buf.resize(buf.len() + 4 * parts.len(), 0);
            for (part, (size, header)) in parts.iter().enumerate().rev() {
                let from = start + part * 0xFF_FF_FF;
                let to = from + 4 * (part + 1);
                buf.copy_within(from..from + size, to);
                buf[to - 4..to].copy_from_slice(header);
            }
        }

        Ok(())
    }
}

impl Packet<Bytes> {
    pub(crate) fn decode<'de, T>(self) -> Result<T, Error>
    where
        T: ProtocolDecode<'de, ()>,
    {
        self.decode_with(())
    }

    pub(crate) fn decode_with<'de, T, C>(self, context: C) -> Result<T, Error>
    where
        T: ProtocolDecode<'de, C>,
    {
        T::decode_with(self.0, context)
    }

    pub(crate) fn ok(self) -> Result<OkPacket, Error> {
        self.decode()
    }

    pub(crate) fn eof(self, capabilities: Capabilities) -> Result<EofPacket, Error> {
        if capabilities.contains(Capabilities::DEPRECATE_EOF) {
            let ok = self.ok()?;

            Ok(EofPacket {
                warnings: ok.warnings,
                status: ok.status,
            })
        } else {
            self.decode_with(capabilities)
        }
    }
}

impl Deref for Packet<Bytes> {
    type Target = Bytes;

    fn deref(&self) -> &Bytes {
        &self.0
    }
}

impl DerefMut for Packet<Bytes> {
    fn deref_mut(&mut self) -> &mut Bytes {
        &mut self.0
    }
}

#[cfg(test)]
#[path = "../../tests/unit/protocol_packet_tests.rs"]
mod tests;
