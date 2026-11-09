use super::*;
use crate::io::ReadBuf;
use crate::net::ScriptedSocket;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::task::{Context, Poll};

fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// A socket whose writes take at most `limit` bytes; it counts the write calls.
struct LimitedWrites {
    limit: usize,
    calls: Arc<AtomicUsize>,
}

impl Socket for LimitedWrites {
    fn try_read(&mut self, _: &mut dyn ReadBuf) -> io::Result<usize> {
        Err(io::ErrorKind::WouldBlock.into())
    }

    fn try_write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(buf.len().min(self.limit))
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

fn limited(limit: usize) -> (BufferedSocket<LimitedWrites>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let socket = LimitedWrites {
        limit,
        calls: calls.clone(),
    };
    (BufferedSocket::new(socket), calls)
}

#[test]
fn shrinking_keeps_a_usable_write_buffer() {
    // (name, bytes written before the shrink, bytes the socket takes per write, flush first?)
    let cases: [(&str, &[u8], usize, bool); 3] = [
        ("untouched", b"", usize::MAX, false),
        ("drained", b"abc", usize::MAX, true),
        ("partially flushed", b"abcdef", 2, false),
    ];

    for (name, pending, limit, flush) in cases {
        let (mut socket, _) = limited(limit);
        socket.write(pending).unwrap();
        if flush {
            run(socket.flush()).unwrap();
        } else if !pending.is_empty() {
            // Take one partial write so the buffer holds flushed and unflushed bytes.
            let written = run(socket.socket.write(socket.write_buf.get())).unwrap();
            socket.write_buf.consume(written);
        }
        let unflushed = socket.write_buf.get().to_vec();

        socket.shrink_buffers();
        assert!(
            socket.write_buf.buf.capacity() >= DEFAULT_BUF_SIZE,
            "{name}: capacity {}",
            socket.write_buf.buf.capacity()
        );
        assert_eq!(socket.write_buf.get(), unflushed, "{name}");

        socket.write(&b"x"[..]).unwrap();
        let mut expected = unflushed;
        expected.push(b'x');
        assert_eq!(socket.write_buf.get(), expected, "{name}");
    }
}

#[test]
fn flush_fails_when_the_socket_takes_no_bytes() {
    let (mut socket, calls) = limited(0);
    socket.write(&b"abc"[..]).unwrap();

    let error = run(socket.flush()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(socket.write_buf.get(), b"abc", "nothing is dropped");
}

#[test]
fn flush_keeps_writing_after_partial_writes() {
    let (mut socket, calls) = limited(2);
    socket.write(&b"abcde"[..]).unwrap();

    run(socket.flush()).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert!(socket.write_buf.is_empty());
}

#[test]
fn flush_of_an_empty_buffer_writes_nothing() {
    let mut socket = BufferedSocket::new(ScriptedSocket::new(Vec::new()));
    run(socket.flush()).unwrap();
    assert!(socket.write_buf.is_empty());
}
