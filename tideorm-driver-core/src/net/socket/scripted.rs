use std::io;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use super::Socket;
use crate::io::ReadBuf;

/// A socket for the drivers' tests: reads return the bytes in `input`, which
/// a test can extend while a read waits, and writes are discarded. With
/// nothing left to read, a read would block.
#[doc(hidden)]
pub struct ScriptedSocket {
    pub input: Arc<Mutex<Vec<u8>>>,
}

impl ScriptedSocket {
    pub fn new(input: Vec<u8>) -> Self {
        Self {
            input: Arc::new(Mutex::new(input)),
        }
    }
}

impl Socket for ScriptedSocket {
    fn try_read(&mut self, buf: &mut dyn ReadBuf) -> io::Result<usize> {
        let mut input = self.input.lock().unwrap_or_else(|e| e.into_inner());
        if input.is_empty() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let n = input.len().min(buf.remaining_mut());
        buf.put_slice(&input[..n]);
        input.drain(..n);
        Ok(n)
    }

    fn try_write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn poll_read_ready(&mut self, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }

    fn poll_write_ready(&mut self, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(&mut self, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
