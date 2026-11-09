mod socket;
pub mod tls;

#[doc(hidden)]
pub use socket::ScriptedSocket;
pub use socket::{
    connect_tcp, connect_uds, BufferedSocket, Socket, SocketIntoBox, WithSocket, WriteBuffer,
};
