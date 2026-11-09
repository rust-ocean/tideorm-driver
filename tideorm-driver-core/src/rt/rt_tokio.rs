//! The Tokio side of the runtime layer: its sockets, and whether a Tokio runtime is running.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::AsyncWrite;
use tokio::net::TcpStream;

use crate::io::ReadBuf;
use crate::net::Socket;

/// Whether the calling thread is inside a Tokio runtime, whose timers and sockets then work.
pub fn available() -> bool {
    tokio::runtime::Handle::try_current().is_ok()
}

// `(*self)` reaches the stream's inherent method, not this trait's method of the same name.
macro_rules! impl_socket {
    ($stream:ty) => {
        impl Socket for $stream {
            fn try_read(&mut self, mut buf: &mut dyn ReadBuf) -> io::Result<usize> {
                // Requires `&mut impl BufMut`
                self.try_read_buf(&mut buf)
            }

            fn try_write(&mut self, buf: &[u8]) -> io::Result<usize> {
                (*self).try_write(buf)
            }

            fn poll_read_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                (*self).poll_read_ready(cx)
            }

            fn poll_write_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                (*self).poll_write_ready(cx)
            }

            fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                Pin::new(self).poll_shutdown(cx)
            }
        }
    };
}

impl_socket!(TcpStream);

#[cfg(unix)]
impl_socket!(tokio::net::UnixStream);
