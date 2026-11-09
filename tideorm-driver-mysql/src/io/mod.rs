//! Reading and writing MySQL frames: the shared field and message traits, plus MySQL's
//! length-encoded integers and strings.

mod buf;
mod buf_mut;

pub use buf::MySqlBufExt;
pub use buf_mut::MySqlBufMutExt;
pub(crate) use driver_core::io::{BufExt, BufMutExt, ProtocolDecode, ProtocolEncode};
