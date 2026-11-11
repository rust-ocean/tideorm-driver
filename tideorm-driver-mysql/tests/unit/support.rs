//! Helpers the unit tests share.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};

/// `payload` framed as a packet with sequence id 0.
pub(crate) fn packet(payload: &[u8]) -> Vec<u8> {
    let mut bytes = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
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
