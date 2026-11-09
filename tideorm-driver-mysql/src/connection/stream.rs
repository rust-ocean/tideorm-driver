use std::collections::VecDeque;
use std::ops::{ControlFlow, Deref, DerefMut};

use bytes::{Buf, Bytes, BytesMut};

use crate::error::Error;
use crate::io::MySqlBufExt;
use crate::io::{ProtocolDecode, ProtocolEncode};
use crate::net::{BufferedSocket, Socket};
use crate::protocol::response::{EofPacket, ErrPacket, Status};
use crate::protocol::{Capabilities, Packet};
use crate::{MySqlConnectOptions, MySqlDatabaseError};

/// The largest packet MySQL sends: `max_allowed_packet` is at most 1 GiB.
const MAX_PACKET_LEN: usize = 1 << 30;

pub struct MySqlStream<S = Box<dyn Socket>> {
    // Wrapping the socket in `Box` allows us to unsize in-place.
    pub(crate) socket: BufferedSocket<S>,
    pub(super) capabilities: Capabilities,
    pub(crate) sequence_id: u8,
    pub(crate) waiting: VecDeque<Waiting>,
    pub(crate) is_tls: bool,
    pub(crate) status_flags: Status,
    /// Set by a deadlock, which rolls the whole transaction back, savepoints
    /// and all; cleared as the next transaction begins.
    pub(crate) deadlocked: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Waiting {
    // waiting for a result set
    Result,

    // waiting for a row within a result set
    Row,

    /// Waiting for the rest of a prepare's response, whose definition
    /// packets nothing here could tell from the next reply's: a prepare
    /// cancelled part-way leaves the connection unusable.
    Prepare,

    /// Reading the parts of a packet of 16 MiB or more, whose later parts
    /// nothing could tell from packets: a read cancelled part-way leaves the
    /// connection unusable.
    Packet,
}

impl<S: Socket> MySqlStream<S> {
    pub(crate) fn with_socket(options: &MySqlConnectOptions, socket: S) -> Self {
        let mut capabilities = Capabilities::PROTOCOL_41
            | Capabilities::IGNORE_SPACE
            | Capabilities::DEPRECATE_EOF
            | Capabilities::FOUND_ROWS
            | Capabilities::TRANSACTIONS
            | Capabilities::SECURE_CONNECTION
            | Capabilities::PLUGIN_AUTH_LENENC_DATA
            | Capabilities::MULTI_STATEMENTS
            | Capabilities::MULTI_RESULTS
            | Capabilities::PLUGIN_AUTH
            | Capabilities::PS_MULTI_RESULTS
            | Capabilities::SSL
            // MariaDB names a `JSON` or `UUID` column's type only in its
            // extended metadata; a MySQL server never offers it.
            | Capabilities::MARIADB_CLIENT_EXTENDED_TYPE_INFO;

        if options.database.is_some() {
            capabilities |= Capabilities::CONNECT_WITH_DB;
        }

        Self {
            waiting: VecDeque::new(),
            capabilities,
            sequence_id: 0,
            socket: BufferedSocket::new(socket),
            is_tls: false,
            status_flags: Status::empty(),
            deadlocked: false,
        }
    }

    pub(crate) async fn wait_until_ready(&mut self) -> Result<(), Error> {
        if !self.socket.write_buffer().is_empty() {
            self.socket.flush().await?;
        }

        while !self.waiting.is_empty() {
            match self.waiting.front() {
                Some(Waiting::Prepare) => {
                    return Err(err_protocol!(
                        "a statement was cancelled while its prepare was answered; \
                         the connection must be closed"
                    ));
                }
                Some(Waiting::Packet) => {
                    return Err(err_protocol!(
                        "a read was cancelled between the parts of a packet; \
                         the connection must be closed"
                    ));
                }
                _ => {}
            }

            while self.waiting.front() == Some(&Waiting::Row) {
                let packet = self.recv_packet().await?;

                if is_terminator(&packet) {
                    let eof = packet.eof(self.capabilities)?;
                    self.status_flags = eof.status;

                    if eof.status.contains(Status::SERVER_MORE_RESULTS_EXISTS) {
                        *self.waiting.front_mut().unwrap() = Waiting::Result;
                    } else {
                        self.waiting.pop_front();
                    };
                }
            }

            while self.waiting.front() == Some(&Waiting::Result) {
                let packet = self.recv_packet().await?;

                if !packet.is_empty() && (packet[0] == 0x00 || packet[0] == 0xff) {
                    let ok = packet.ok()?;
                    self.status_flags = ok.status;

                    if !ok.status.contains(Status::SERVER_MORE_RESULTS_EXISTS) {
                        self.waiting.pop_front();
                    }
                } else {
                    *self.waiting.front_mut().unwrap() = Waiting::Row;
                    self.skip_result_metadata(packet).await?;
                }
            }
        }

        Ok(())
    }

    pub(crate) async fn send_packet<'en, T>(&mut self, payload: T) -> Result<(), Error>
    where
        T: ProtocolEncode<'en, Capabilities>,
    {
        self.write_command(payload)?;
        self.flush().await?;
        Ok(())
    }

    /// Write a command to go out with the next flush.
    pub(crate) fn write_command<'en, T>(&mut self, payload: T) -> Result<(), Error>
    where
        T: ProtocolEncode<'en, Capabilities>,
    {
        self.sequence_id = 0;
        self.write_packet(payload)
    }

    pub(crate) fn write_packet<'en, T>(&mut self, payload: T) -> Result<(), Error>
    where
        T: ProtocolEncode<'en, Capabilities>,
    {
        self.socket
            .write_with(Packet(payload), (self.capabilities, &mut self.sequence_id))
    }

    async fn recv_packet_part(&mut self) -> Result<Bytes, Error> {
        let (sequence_id, payload) = self
            .socket
            .try_read(|buf| {
                let Some(header) = buf.get(..4) else {
                    return Ok(ControlFlow::Continue(4));
                };
                let packet_size =
                    usize::try_from(u32::from_le_bytes([header[0], header[1], header[2], 0]))
                        .map_err(|_| err_protocol!("packet size overflows usize"))?;
                let sequence_id = header[3];
                if buf.len() < packet_size + 4 {
                    return Ok(ControlFlow::Continue(packet_size + 4));
                }
                // Keep the header until the payload is available so cancellation
                // leaves a complete packet for the next reader.
                buf.advance(4);
                Ok(ControlFlow::Break((
                    sequence_id,
                    buf.split_to(packet_size).freeze(),
                )))
            })
            .await?;
        self.sequence_id = sequence_id.wrapping_add(1);
        Ok(payload)
    }

    // receive the next packet from the database server
    // may block (async) on more data from the server
    pub(crate) async fn recv_packet(&mut self) -> Result<Packet<Bytes>, Error> {
        let payload = self.recv_packet_part().await?;
        let payload = if payload.len() < 0xFF_FF_FF {
            payload
        } else {
            // taken off once the last part is read
            self.waiting.push_front(Waiting::Packet);

            let mut final_payload = BytesMut::with_capacity(0xFF_FF_FF * 2);
            final_payload.extend_from_slice(&payload);

            drop(payload); // we don't need the allocation anymore

            let mut last_read = 0xFF_FF_FF;
            while last_read == 0xFF_FF_FF {
                let part = self.recv_packet_part().await?;
                last_read = part.len();
                if final_payload.len() + last_read > MAX_PACKET_LEN {
                    return Err(err_protocol!("packet larger than {MAX_PACKET_LEN} bytes"));
                }
                final_payload.extend_from_slice(&part);
            }

            self.waiting.pop_front();
            final_payload.into()
        };

        if payload
            .first()
            .ok_or(err_protocol!("Packet empty"))?
            .eq(&0xff)
        {
            self.waiting.pop_front();

            // instead of letting this packet be looked at everywhere, we check here
            // and emit a proper Error
            let error = ErrPacket::decode_with(payload, self.capabilities)?;
            self.deadlocked |= error.error_code == crate::error::error_codes::ER_LOCK_DEADLOCK;
            return Err(MySqlDatabaseError(error).into());
        }

        Ok(Packet(payload))
    }

    pub(crate) async fn recv<'de, T>(&mut self) -> Result<T, Error>
    where
        T: ProtocolDecode<'de, Capabilities>,
    {
        self.recv_packet().await?.decode_with(self.capabilities)
    }

    pub(crate) async fn maybe_recv_eof(&mut self) -> Result<Option<EofPacket>, Error> {
        if self.capabilities.contains(Capabilities::DEPRECATE_EOF) {
            Ok(None)
        } else {
            let eof: EofPacket = self.recv().await?;
            self.status_flags = eof.status;
            Ok(Some(eof))
        }
    }

    async fn skip_result_metadata(&mut self, mut packet: Packet<Bytes>) -> Result<(), Error> {
        let num_columns: u64 = packet.get_uint_lenenc()?; // column count

        for _ in 0..num_columns {
            let _ = self.recv_packet().await?;
        }

        self.maybe_recv_eof().await?;

        Ok(())
    }

    pub fn boxed_socket(self) -> MySqlStream {
        MySqlStream {
            socket: self.socket.boxed(),
            capabilities: self.capabilities,
            sequence_id: self.sequence_id,
            waiting: self.waiting,
            is_tls: self.is_tls,
            status_flags: self.status_flags,
            deadlocked: self.deadlocked,
        }
    }
}

/// Whether `packet` ends a result set: an EOF packet, or the OK packet that
/// stands for one. A text row whose first value has 16 MiB or more starts
/// with 0xfe too, and is longer.
pub(crate) fn is_terminator(packet: &[u8]) -> bool {
    packet.first() == Some(&0xfe) && packet.len() < 0xFF_FF_FF
}

impl<S> Deref for MySqlStream<S> {
    type Target = BufferedSocket<S>;

    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

impl<S> DerefMut for MySqlStream<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.socket
    }
}

#[cfg(test)]
#[path = "../../tests/unit/connection_stream_tests.rs"]
mod tests;
