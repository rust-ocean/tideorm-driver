//! The async-io side of the runtime layer, used with async-std: sockets registered with its
//! reactor, and its timers.

use std::future::Future;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::pin::pin;
use std::task::{Context, Poll};
use std::time::Duration;

use async_io::{Async, Timer};
use futures_util::future::{select, Either};

use crate::io::ReadBuf;
use crate::net::Socket;
use crate::rt::TimeoutError;

pub async fn sleep(duration: Duration) {
    Timer::after(duration).await;
}

/// Runs `future` until it finishes or `duration` passes, whichever is first.
pub async fn timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, TimeoutError> {
    match select(pin!(future), Timer::after(duration)).await {
        Either::Left((output, _)) => Ok(output),
        Either::Right(_) => Err(TimeoutError),
    }
}

macro_rules! impl_socket {
    ($stream:ty) => {
        impl Socket for Async<$stream> {
            fn try_read(&mut self, buf: &mut dyn ReadBuf) -> io::Result<usize> {
                self.get_ref().read(buf.init_mut())
            }

            fn try_write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.get_ref().write(buf)
            }

            fn poll_read_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                self.poll_readable(cx)
            }

            fn poll_write_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                self.poll_writable(cx)
            }

            fn poll_shutdown(&mut self, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                Poll::Ready(self.get_ref().shutdown(Shutdown::Both))
            }
        }
    };
}

impl_socket!(TcpStream);

#[cfg(unix)]
impl_socket!(std::os::unix::net::UnixStream);
