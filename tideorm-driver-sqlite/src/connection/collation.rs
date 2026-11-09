//! Application-defined collations: a Rust closure SQLite calls to order two strings,
//! registered with [`sqlite3_create_collation_v2`].
//!
//! See <https://www.sqlite.org/c3ref/create_collation.html>.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::ffi::CString;
use std::fmt::{self, Debug, Formatter};
use std::os::raw::{c_int, c_void};
use std::slice;
use std::sync::Arc;

use libsqlite3_sys::{sqlite3_create_collation_v2, SQLITE_OK, SQLITE_UTF8};

use crate::connection::handle::ConnectionHandle;
use crate::error::Error;

/// A named comparison that connect options keep and register on every connection they
/// open.
#[derive(Clone)]
pub struct Collation {
    name: Arc<str>,
    #[allow(clippy::type_complexity)]
    collate: Arc<dyn Fn(&str, &str) -> Ordering + Send + Sync + 'static>,
    // SAFETY: `new` instantiates both with the concrete closure type behind `collate`, the
    // type of the value the pointer handed to SQLite points to.
    call: unsafe extern "C" fn(
        arg1: *mut c_void,
        arg2: c_int,
        arg3: *const c_void,
        arg4: c_int,
        arg5: *const c_void,
    ) -> c_int,
    free: unsafe extern "C" fn(*mut c_void),
}

impl Collation {
    pub fn new<N, F>(name: N, collate: F) -> Self
    where
        N: Into<Arc<str>>,
        F: Fn(&str, &str) -> Ordering + Send + Sync + 'static,
    {
        /// Gives back the reference count `create` handed to SQLite.
        unsafe extern "C" fn drop_arc_value<T>(p: *mut c_void) {
            drop(Arc::from_raw(p as *mut T));
        }

        Collation {
            name: name.into(),
            collate: Arc::new(collate),
            call: compare::<F>,
            free: drop_arc_value::<F>,
        }
    }

    /// Registers the collation on one connection, replacing one of the same name.
    ///
    /// SQLite keeps a reference to the closure until the collation is replaced or the
    /// connection closes, and then calls `free` to release it.
    pub(crate) fn create(&self, handle: &mut ConnectionHandle) -> Result<(), Error> {
        let c_name = CString::new(&*self.name)
            .map_err(|_| err_protocol!("invalid collation name: {:?}", self.name))?;
        let raw_f = Arc::into_raw(Arc::clone(&self.collate));
        // SAFETY: the handle is open and borrowed mutably; `raw_f` points to the closure
        // `call` and `free` were instantiated for (see the field comment).
        let r = unsafe {
            sqlite3_create_collation_v2(
                handle.as_ptr(),
                c_name.as_ptr(),
                SQLITE_UTF8,
                raw_f as *mut c_void,
                Some(self.call),
                Some(self.free),
            )
        };

        if r == SQLITE_OK {
            Ok(())
        } else {
            // SAFETY: SQLite does not call `free` when registration fails, so the count
            // `raw_f` stands for is still ours to give back.
            drop(unsafe { Arc::from_raw(raw_f) });
            Err(handle.expect_error().into())
        }
    }
}

impl Debug for Collation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Collation")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Registers `compare` under `name` on one connection, as
/// `SqliteConnection::create_collation` does.
pub(crate) fn create_collation<F>(
    handle: &mut ConnectionHandle,
    name: &str,
    compare: F,
) -> Result<(), Error>
where
    F: Fn(&str, &str) -> Ordering + Send + Sync + 'static,
{
    Collation::new(name, compare).create(handle)
}

/// The comparison SQLite calls with the closure `data` points to and the two strings to
/// order, as `(length, pointer)` pairs.
///
/// # Safety
///
/// `data` must point to a live `C`, and each pointer to as many readable bytes as its
/// length says.
unsafe extern "C" fn compare<C>(
    data: *mut c_void,
    left_len: c_int,
    left_ptr: *const c_void,
    right_len: c_int,
    right_ptr: *const c_void,
) -> c_int
where
    C: Fn(&str, &str) -> Ordering,
{
    // A panic cannot unwind into SQLite: it aborts the process instead.
    // https://doc.rust-lang.org/nomicon/ffi.html#ffi-and-unwinding
    assert!(!data.is_null());

    let compare = &*(data as *const C);
    compare(
        &lossy_text(left_ptr, left_len),
        &lossy_text(right_ptr, right_len),
    ) as c_int
}

/// The text SQLite hands a collation, which SQLite documents may be invalid UTF-8; the
/// closure's `&str` gets such bytes replaced with U+FFFD.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes.
unsafe fn lossy_text<'a>(ptr: *const c_void, len: c_int) -> Cow<'a, str> {
    let len = usize::try_from(len).unwrap_or_else(|_| panic!("text length out of range: {len}"));
    String::from_utf8_lossy(slice::from_raw_parts(ptr as *const u8, len))
}

#[cfg(test)]
#[path = "../../tests/unit/connection_collation_tests.rs"]
mod tests;
