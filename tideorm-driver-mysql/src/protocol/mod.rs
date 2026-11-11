//! The packets of MySQL's client/server protocol that the driver reads and
//! writes, which MariaDB speaks too.
//!
//! <https://dev.mysql.com/doc/dev/mysql-server/latest/PAGE_PROTOCOL.html>

pub(crate) mod auth;
mod capabilities;
mod command;
mod packet;
mod row;

pub(crate) use capabilities::Capabilities;
pub(crate) use command::Command;
pub(crate) use packet::Packet;
pub(crate) use row::Row;

/// Opening a connection: the server's greeting, the client's answer, and any
/// switch to another authentication plugin.
///
/// <https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase.html>
pub(crate) mod connect {
    mod auth_switch;
    mod handshake;
    mod handshake_response;

    pub(crate) use auth_switch::{AuthSwitchRequest, AuthSwitchResponse};
    pub(crate) use handshake::Handshake;
    pub(crate) use handshake_response::{HandshakeResponse, SslRequest};
}

/// The OK, ERR and EOF packets that end a reply, and the server status they
/// carry.
///
/// <https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_basic_response_packets.html>
pub(crate) mod response {
    mod eof;
    mod err;
    mod ok;
    mod status;

    pub(crate) use eof::EofPacket;
    pub(crate) use err::ErrPacket;
    pub(crate) use ok::OkPacket;
    pub(crate) use status::Status;
}

/// The replies to a prepared statement: its `COM_STMT_PREPARE_OK` and the
/// rows of the binary format.
pub(crate) mod statement {
    mod prepare_ok;
    mod row;

    pub(crate) use prepare_ok::PrepareOk;
    pub(crate) use row::BinaryRow;
}

/// Column definitions, and the rows of the text format.
pub(crate) mod text {
    mod column;
    mod row;

    pub(crate) use column::{ColumnDefinition, ColumnFlags, ColumnType, ExtendedType};
    pub(crate) use row::TextRow;
}

#[cfg(test)]
#[path = "../../tests/unit/protocol_fuzz_tests.rs"]
mod fuzz_tests;
