//! The commands the driver sends once connected. Each is one packet: a
//! command byte, then that command's arguments.
//!
//! <https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_command_phase.html>

use crate::error::Error;
use crate::io::ProtocolEncode;
use crate::protocol::text::ColumnFlags;
use crate::protocol::Capabilities;
use crate::MySqlArguments;

const COM_QUIT: u8 = 0x01;
const COM_QUERY: u8 = 0x03;
const COM_PING: u8 = 0x0e;
const COM_STMT_PREPARE: u8 = 0x16;
const COM_STMT_EXECUTE: u8 = 0x17;
const COM_STMT_CLOSE: u8 = 0x19;

/// No cursor: the whole result set follows the execute.
const CURSOR_TYPE_NO_CURSOR: u8 = 0;

/// Marks an unsigned parameter in an execute's type list.
const UNSIGNED_PARAMETER: u8 = 0x80;

#[derive(Debug)]
pub(crate) enum Command<'a> {
    /// Tells the server the client is leaving; no reply.
    Quit,
    /// Runs SQL text; the reply is read in the text format.
    Query(&'a str),
    /// Answered with an OK packet.
    Ping,
    /// Prepares SQL for `Execute`; answered with a `PrepareOk`.
    Prepare(&'a str),
    /// Runs a prepared statement; the reply is read in the binary format.
    Execute {
        statement: u32,
        arguments: &'a MySqlArguments,
    },
    /// Drops a prepared statement; no reply.
    CloseStatement(u32),
}

impl ProtocolEncode<'_, Capabilities> for Command<'_> {
    fn encode_with(&self, buf: &mut Vec<u8>, _: Capabilities) -> Result<(), Error> {
        match *self {
            Command::Quit => buf.push(COM_QUIT),
            Command::Ping => buf.push(COM_PING),
            Command::Query(sql) => {
                buf.push(COM_QUERY);
                buf.extend_from_slice(sql.as_bytes());
            }
            Command::Prepare(sql) => {
                buf.push(COM_STMT_PREPARE);
                buf.extend_from_slice(sql.as_bytes());
            }
            Command::CloseStatement(statement) => {
                buf.push(COM_STMT_CLOSE);
                buf.extend_from_slice(&statement.to_le_bytes());
            }
            Command::Execute {
                statement,
                arguments,
            } => {
                buf.push(COM_STMT_EXECUTE);
                buf.extend_from_slice(&statement.to_le_bytes());
                buf.push(CURSOR_TYPE_NO_CURSOR);
                // iteration count, always 1
                buf.extend_from_slice(&1_u32.to_le_bytes());

                // A statement with no parameters ends here; otherwise the NULL
                // bitmap, the "types follow" flag, each parameter's type and
                // its sign, then the values.
                if !arguments.types.is_empty() {
                    buf.extend_from_slice(&arguments.null_bitmap);
                    buf.push(1);
                    for ty in &arguments.types {
                        let sign = if ty.flags.contains(ColumnFlags::UNSIGNED) {
                            UNSIGNED_PARAMETER
                        } else {
                            0
                        };
                        buf.extend_from_slice(&[ty.r#type as u8, sign]);
                    }
                    buf.extend_from_slice(&arguments.values);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/protocol_command_tests.rs"]
mod tests;
