//! File reads that do not block the async runtime.

use std::io;
use std::path::{Path, PathBuf};

use crate::rt;

/// Reads the whole file at `path` (a TLS certificate or key, for instance).
///
/// Operating systems offer no portable non-blocking file I/O, so the read runs on the
/// runtime's blocking thread pool, which is all the runtimes' own `fs` modules do too.
pub async fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let path = PathBuf::from(path.as_ref());
    rt::spawn_blocking(move || std::fs::read(path)).await
}
