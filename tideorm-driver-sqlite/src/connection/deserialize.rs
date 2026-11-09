//! Copying a whole database into memory and back: [`sqlite3_serialize`] and
//! [`sqlite3_deserialize`], with the SQLite-allocated buffer they exchange.
//!
//! See <https://sqlite.org/c3ref/serialize.html> and
//! <https://sqlite.org/c3ref/deserialize.html>.

use super::ConnectionState;
use crate::{error::Error, SqliteConnection, SqliteError};
use libsqlite3_sys::{
    sqlite3_deserialize, sqlite3_free, sqlite3_malloc64, sqlite3_serialize,
    SQLITE_DESERIALIZE_FREEONCLOSE, SQLITE_DESERIALIZE_READONLY, SQLITE_DESERIALIZE_RESIZEABLE,
    SQLITE_NOMEM, SQLITE_OK,
};
use std::ffi::CString;
use std::{
    ops::{Deref, DerefMut},
    ptr,
    ptr::NonNull,
};

impl SqliteConnection {
    /// Copies the database attached as `schema` (`None` for `main`) into a buffer, with
    /// [`sqlite3_serialize()`]: the same bytes its file on disk would hold.
    ///
    /// SQLite allocates the buffer; it is freed when the returned value drops.
    ///
    /// # Errors
    /// * [`Error::InvalidArgument`] if `schema` contains a NUL byte (`\0`).
    /// * [`Error::Database`] if no database is attached under `schema`, or SQLite fails.
    ///
    /// [`sqlite3_serialize()`]: https://sqlite.org/c3ref/serialize.html
    pub async fn serialize(&mut self, schema: Option<&str>) -> Result<SqliteOwnedBuf, Error> {
        let schema = schema.map(SchemaName::try_from).transpose()?;

        self.worker.serialize(schema).await
    }

    /// Replaces the database attached as `schema` (`None` for `main`) with an in-memory one
    /// read from `data`, with [`sqlite3_deserialize()`]. `data` is normally what
    /// [`Self::serialize()`] returned: a database file's bytes.
    ///
    /// The schema must already be attached; `ATTACH ':memory:' AS "<schema name>"` attaches
    /// an empty one.
    ///
    /// The connection takes `data` over and frees it when it closes or detaches the schema
    /// ([`SQLITE_DESERIALIZE_FREEONCLOSE`][deserialize-flags]). With `read_only` the
    /// database cannot be written ([`SQLITE_DESERIALIZE_READONLY`][deserialize-flags]);
    /// without it, it may grow past `data`
    /// ([`SQLITE_DESERIALIZE_RESIZEABLE`][deserialize-flags]).
    ///
    /// SQLite cannot open a deserialized database in WAL mode, so an image whose file format
    /// version bytes (18 and 19 of the header) say WAL is refused before the schema is
    /// touched. Setting both bytes to 1 loads it in rollback mode instead.
    ///
    /// # Errors
    /// * [`Error::InvalidArgument`] if `schema` contains a NUL byte (`\0`), or `data` is a
    ///   WAL image.
    /// * [`Error::Database`] if SQLite refuses the data or the schema.
    ///
    /// [`sqlite3_deserialize()`]: https://sqlite.org/c3ref/deserialize.html
    /// [deserialize-flags]: https://sqlite.org/c3ref/c_deserialize_freeonclose.html
    pub async fn deserialize(
        &mut self,
        schema: Option<&str>,
        data: SqliteOwnedBuf,
        read_only: bool,
    ) -> Result<(), Error> {
        let schema = schema.map(SchemaName::try_from).transpose()?;

        // A version byte of 2 marks WAL: https://www.sqlite.org/fileformat2.html#file_format_version_numbers
        if data
            .get(18..20)
            .is_some_and(|versions| versions.contains(&2))
        {
            return Err(Error::InvalidArgument(
                "the database image is in WAL mode (header byte 18 or 19 is 2), which SQLite \
                 cannot open deserialized; set both bytes to 1 first"
                    .to_string(),
            ));
        }

        self.worker.deserialize(schema, data, read_only).await
    }
}

pub(crate) fn serialize(
    conn: &mut ConnectionState,
    schema: Option<SchemaName>,
) -> Result<SqliteOwnedBuf, Error> {
    let mut size = 0;

    // SAFETY: the handle is open and owned by this worker; the schema name is
    // NUL-terminated; with no flags SQLite returns a fresh `sqlite3_malloc` buffer of
    // `size` bytes (or NULL), which `SqliteOwnedBuf` then owns.
    let buf = unsafe {
        let ptr = sqlite3_serialize(
            conn.handle.as_ptr(),
            schema.as_ref().map_or(ptr::null(), |name| name.0.as_ptr()),
            &mut size,
            0,
        );

        // SQLite's memdb.c sets `size` to -1 when it fails.
        usize::try_from(size)
            .ok()
            .and_then(|size| SqliteOwnedBuf::from_raw(ptr, size))
    };

    if let Some(buf) = buf {
        return Ok(buf);
    }

    if let Some(error) = conn.handle.last_error() {
        return Err(error.into());
    }

    if size > 0 {
        // SQLite knew the size but returned no buffer: the allocation failed.
        return Err(SqliteError::from_code(SQLITE_NOMEM).into());
    }

    // No error and no size: there is no such schema. Report it as SQLite reports
    // `PRAGMA <schema>.page_count` on a missing one.
    Err(SqliteError::generic(format!(
        "database {} does not exist",
        schema
            .as_ref()
            .map_or("main".into(), |name| name.0.to_string_lossy())
    ))
    .into())
}

pub(crate) fn deserialize(
    conn: &mut ConnectionState,
    schema: Option<SchemaName>,
    data: SqliteOwnedBuf,
    read_only: bool,
) -> Result<(), Error> {
    let flags = SQLITE_DESERIALIZE_FREEONCLOSE
        | if read_only {
            SQLITE_DESERIALIZE_READONLY
        } else {
            SQLITE_DESERIALIZE_RESIZEABLE
        };

    let (buf, size, capacity) = data.into_raw();
    let size = i64::try_from(size).unwrap();
    let capacity = i64::try_from(capacity).unwrap();

    // SAFETY: the handle is open and owned by this worker; the schema name is
    // NUL-terminated; `buf` is a `sqlite3_malloc` allocation of `capacity` initialized
    // bytes, the image in its first `size`, that nothing else owns any more. With
    // `FREEONCLOSE` SQLite frees it, even when this call fails.
    let rc = unsafe {
        sqlite3_deserialize(
            conn.handle.as_ptr(),
            schema.as_ref().map_or(ptr::null(), |name| name.0.as_ptr()),
            buf,
            size,
            capacity,
            flags,
        )
    };

    match rc {
        SQLITE_OK => Ok(()),
        SQLITE_NOMEM => Err(SqliteError::from_code(SQLITE_NOMEM).into()),
        // SQLite leaves no message for a failed deserialization.
        _ => Err(SqliteError::generic("an error occurred during deserialization").into()),
    }
}

/// Bytes in a buffer SQLite allocated (`sqlite3_malloc`), freed with `sqlite3_free` on
/// drop: what [`SqliteConnection::serialize()`] returns and
/// [`SqliteConnection::deserialize()`] takes.
///
/// `TryFrom<&[u8]>` copies a non-empty slice into a new one, followed by zeroed padding:
/// SQLite may read a little past a malformed image, and advises padding an image it
/// cannot trust (<https://sqlite.org/c3ref/deserialize.html>). A buffer `serialize()`
/// returns is SQLite's own image and carries none.
#[derive(Debug)]
pub struct SqliteOwnedBuf {
    ptr: NonNull<u8>,
    /// The bytes of the image, all the slice shows.
    size: usize,
    /// The bytes allocated: `size`, plus the padding of a copy.
    capacity: usize,
}

/// The zeroed bytes a copy allocates past its image, as SQLite's documentation advises.
const PADDING: usize = 20;

// SAFETY: the value owns its buffer outright, and SQLite's allocator may free it from any
// thread; shared access only reads it.
unsafe impl Send for SqliteOwnedBuf {}
unsafe impl Sync for SqliteOwnedBuf {}

impl Drop for SqliteOwnedBuf {
    fn drop(&mut self) {
        // SAFETY: `ptr` came from SQLite's allocator and only this value frees it.
        unsafe {
            sqlite3_free(self.ptr.as_ptr().cast());
        }
    }
}

impl SqliteOwnedBuf {
    /// Takes ownership of `size` initialized bytes at `ptr`; `None` for a NULL `ptr`.
    ///
    /// # Safety
    /// `ptr` must be NULL or a `sqlite3_malloc` allocation of at least `size` initialized
    /// bytes that nothing else frees.
    unsafe fn from_raw(ptr: *mut u8, size: usize) -> Option<Self> {
        Some(Self {
            ptr: NonNull::new(ptr)?,
            size,
            capacity: size,
        })
    }

    /// Gives the buffer up without freeing it, for SQLite to take over: its pointer, image
    /// size and allocated size.
    fn into_raw(self) -> (*mut u8, usize, usize) {
        let raw = (self.ptr.as_ptr(), self.size, self.capacity);
        std::mem::forget(self);
        raw
    }
}

/// # Errors
/// * [`Error::InvalidArgument`] if the slice is empty.
/// * [`Error::Database`] if SQLite's allocator is out of memory.
impl TryFrom<&[u8]> for SqliteOwnedBuf {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.is_empty() {
            return Err(Error::InvalidArgument(
                "SQLite owned buffer cannot be empty".to_string(),
            ));
        }

        // A slice holds at most `isize::MAX` bytes, so this cannot overflow.
        let capacity = bytes.len() + PADDING;
        // SAFETY: `sqlite3_malloc64` returns NULL or `capacity` bytes nothing else owns;
        // the copy writes the first `bytes.len()` and the zeroing the rest before the
        // buffer is read.
        unsafe {
            let ptr = sqlite3_malloc64(u64::try_from(capacity).unwrap()).cast::<u8>();
            let ptr = NonNull::new(ptr)
                .ok_or_else(|| Error::from(SqliteError::from_code(SQLITE_NOMEM)))?;
            ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.as_ptr(), bytes.len());
            ptr::write_bytes(ptr.as_ptr().add(bytes.len()), 0, PADDING);
            Ok(Self {
                ptr,
                size: bytes.len(),
                capacity,
            })
        }
    }
}

impl Deref for SqliteOwnedBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        // SAFETY: `ptr` owns `size` initialized bytes for as long as `self` lives.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.size) }
    }
}

impl DerefMut for SqliteOwnedBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: as for `deref`, and `&mut self` makes the access exclusive.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_mut(), self.size) }
    }
}

impl AsRef<[u8]> for SqliteOwnedBuf {
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl AsMut<[u8]> for SqliteOwnedBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        self.deref_mut()
    }
}

/// A schema name ready for SQLite: checked for NUL bytes and NUL-terminated.
#[derive(Debug)]
pub(crate) struct SchemaName(CString);

impl<'a> TryFrom<&'a str> for SchemaName {
    type Error = Error;

    fn try_from(name: &'a str) -> Result<Self, Self::Error> {
        CString::new(name).map(SchemaName).map_err(|e| {
            Error::InvalidArgument(format!(
                "schema name {name:?} contains a zero byte at index {}",
                e.nul_position()
            ))
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/connection_deserialize_tests.rs"]
mod tests;
