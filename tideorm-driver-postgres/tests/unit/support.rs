//! Helpers the unit tests share.

use crate::encode::Encode;
use crate::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A value in the binary format, as a row gives it.
pub(crate) fn binary(bytes: &[u8], type_info: PgTypeInfo) -> PgValueRef<'_> {
    PgValueRef {
        value: Some(bytes),
        row: None,
        type_info,
        format: PgValueFormat::Binary,
    }
}

/// A value in the text format, as a row gives it.
pub(crate) fn text(value: &str, type_info: PgTypeInfo) -> PgValueRef<'_> {
    PgValueRef {
        value: Some(value.as_bytes()),
        row: None,
        type_info,
        format: PgValueFormat::Text,
    }
}

/// The bytes `value` encodes to.
pub(crate) fn encoded<'q, T: Encode<'q, Postgres>>(value: &T) -> Vec<u8> {
    let mut buf = PgArgumentBuffer::default();
    assert!(!value.encode_by_ref(&mut buf).unwrap().is_null());
    buf.to_vec()
}

/// A PostgreSQL server on a loopback port for the unit tests. It accepts any startup, answers
/// every simple query as `answer` says, and records each one as `"<session>:<sql>"`.
pub(crate) struct FakeServer {
    pub(crate) url: String,
    log: Arc<Mutex<Vec<String>>>,
}

/// How [`FakeServer`] answers a query.
pub(crate) enum Answer {
    /// `CommandComplete`.
    Done,
    /// An `ErrorResponse`.
    Fail,
    /// `CommandComplete`, then the server ends the session.
    DoneThenClose,
    /// `CommandComplete`, once the flag is set.
    DoneAfter(Arc<AtomicBool>),
}

impl FakeServer {
    pub(crate) fn start(answer: impl Fn(&str) -> Answer + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let records = log.clone();
        // The thread ends with the test process; sessions are served one at a time.
        thread::spawn(move || {
            for (session, stream) in listener.incoming().enumerate() {
                let _ = serve(stream, session + 1, &answer, &records);
            }
        });
        Self {
            url: format!("postgres://test@{address}/test?sslmode=disable"),
            log,
        }
    }

    pub(crate) fn queries(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

fn serve(
    stream: io::Result<TcpStream>,
    session: usize,
    answer: &dyn Fn(&str) -> Answer,
    log: &Mutex<Vec<String>>,
) -> io::Result<()> {
    let mut stream = stream?;
    read_body(&mut stream)?; // the startup message
    send(&mut stream, b'R', &0u32.to_be_bytes())?; // AuthenticationOk
    send(&mut stream, b'S', b"server_version\x0016.0\x00")?;
    send(&mut stream, b'Z', b"I")?;
    loop {
        let mut tag = [0];
        stream.read_exact(&mut tag)?;
        let body = read_body(&mut stream)?;
        match tag[0] {
            b'X' => return Ok(()),
            b'S' => send(&mut stream, b'Z', b"I")?,
            b'Q' => {
                let sql = String::from_utf8_lossy(body.strip_suffix(b"\0").unwrap_or(&body));
                log.lock().unwrap().push(format!("{session}:{sql}"));
                let answer = answer(&sql);
                if let Answer::DoneAfter(flag) = &answer {
                    while !flag.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
                match answer {
                    Answer::Fail => send(&mut stream, b'E', b"SERROR\0C42601\0Mrefused\0\0")?,
                    _ => send(&mut stream, b'C', b"SELECT 1\0")?,
                }
                send(&mut stream, b'Z', b"I")?;
                if let Answer::DoneThenClose = answer {
                    return Ok(());
                }
            }
            _ => return Err(io::ErrorKind::InvalidData.into()),
        }
    }
}

fn read_body(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut len = [0; 4];
    stream.read_exact(&mut len)?;
    let len = usize::try_from(u32::from_be_bytes(len)).unwrap();
    let mut body = vec![0; len.saturating_sub(4)];
    stream.read_exact(&mut body)?;
    Ok(body)
}

fn send(stream: &mut TcpStream, tag: u8, body: &[u8]) -> io::Result<()> {
    let len = u32::try_from(body.len() + 4).unwrap();
    stream.write_all(&[tag])?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(body)
}
