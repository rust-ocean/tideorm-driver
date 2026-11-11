//! Helpers the unit tests share.

use crate::encode::Encode;
use crate::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::panic::{self, AssertUnwindSafe};
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

/// A deterministic xorshift64 generator, so a failing input is the same on every run.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        // xorshift never leaves zero
        Self(seed | 1)
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A number below `n`, which is not 0.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }

    pub(crate) fn byte(&mut self) -> u8 {
        self.next().to_le_bytes()[0]
    }

    /// Up to `max` random bytes.
    pub(crate) fn bytes(&mut self, max: usize) -> Vec<u8> {
        let len = self.below(max + 1);
        (0..len).map(|_| self.byte()).collect()
    }

    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// `valid` after one to four mutations: a truncation, appended garbage, a flipped bit, a
/// replaced byte, a marker byte, a small number, or, most often, a length or count field forced
/// to zero, -1, or the largest or smallest value of its width.
pub(crate) fn mutate(rng: &mut Rng, valid: &[u8]) -> Vec<u8> {
    let mut bytes = valid.to_vec();
    for _ in 0..=rng.below(4) {
        let len = bytes.len();
        match rng.below(9) {
            0 => bytes.truncate(rng.below(len + 1)),
            1 => bytes.extend(rng.bytes(32)),
            _ if len == 0 => bytes.push(rng.byte()),
            2 => bytes[rng.below(len)] ^= 1 << rng.below(8),
            3 => bytes[rng.below(len)] = rng.byte(),
            // NUL ends a string; 0xfb to 0xff lead MySQL's length-encoded integers and packets
            4 => bytes[rng.below(len)] = *rng.pick(&[0, 0xfb, 0xfc, 0xfd, 0xfe, 0xff]),
            // a length a few bytes off the real one
            5 => bytes[rng.below(len)] = rng.byte() % 16,
            _ => {
                let (lead, fill) = *rng.pick(&[(0, 0), (0xff, 0xff), (0x7f, 0xff), (0x80, 0)]);
                let mut field = vec![fill; *rng.pick(&[1, 2, 3, 4, 8])];
                field[0] = lead;
                if rng.below(2) == 0 {
                    field.reverse(); // little-endian
                }
                let at = rng.below(len);
                let end = len.min(at + field.len());
                bytes[at..end].copy_from_slice(&field[..end - at]);
            }
        }
    }
    bytes
}

/// The largest single allocation an input may make: a forged length asks for far more.
const ALLOCATION_LIMIT: usize = 64 << 20;

/// Runs `check` on each of `valid` and on no bytes at all, then on `iterations` more inputs: a
/// quarter of them random bytes, the rest mutations of `valid`. It fails on the first input that
/// panics or makes an allocation past [`ALLOCATION_LIMIT`], naming the input.
pub(crate) fn fuzz(seed: u64, iterations: usize, valid: &[Vec<u8>], mut check: impl FnMut(&[u8])) {
    let mut rng = Rng::new(seed);
    for i in 0..=valid.len() + iterations {
        let input = match valid.get(i) {
            Some(input) => input.clone(),
            None if i == valid.len() => Vec::new(),
            None if valid.is_empty() || rng.below(4) == 0 => rng.bytes(512),
            None => {
                let base = rng.pick(valid);
                mutate(&mut rng, base)
            }
        };
        LARGEST_ALLOCATION.with(|largest| largest.set(0));
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| check(&input)));
        let largest = LARGEST_ALLOCATION.with(Cell::get);
        assert!(
            outcome.is_ok(),
            "panicked on input {i} of seed {seed}: {input:02x?}"
        );
        assert!(
            largest <= ALLOCATION_LIMIT,
            "input {i} of seed {seed} allocated {largest} bytes at once: {input:02x?}"
        );
    }
}

thread_local! {
    static LARGEST_ALLOCATION: Cell<usize> = const { Cell::new(0) };
}

/// The test binary's allocator: the system's, recording each thread's largest request.
struct Recording;

fn record(size: usize) {
    // `try_with`: threads also allocate while their locals are torn down
    let _ = LARGEST_ALLOCATION.try_with(|largest| largest.set(largest.get().max(size)));
}

unsafe impl GlobalAlloc for Recording {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: Recording = Recording;
