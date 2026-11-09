//! Waiting out a shared-cache lock: when a step fails with `SQLITE_LOCKED_SHAREDCACHE`,
//! the caller registers with [`sqlite3_unlock_notify`] and blocks until SQLite reports that
//! the connection holding the lock has finished its transaction.
//!
//! See <https://www.sqlite.org/unlock_notify.html>.
//!
//! The wait has no bound but the other transaction. `busy_timeout` does not apply: it
//! governs `SQLITE_BUSY`, not a shared-cache `SQLITE_LOCKED`. Cancelling the statement's
//! future does not end it either, since the connection's worker thread is the one blocked
//! here; and closing the connection queues behind the blocked statement, so it too waits
//! until the transaction holding the lock ends. A timeout around the condition variable
//! is no fix: SQLite holds a pointer to the waiter's stack storage until the callback
//! fires, so the registration would have to be cancelled first, on this same thread.

use std::ffi::c_void;
use std::os::raw::c_int;
use std::slice;
use std::sync::{Condvar, Mutex};

use libsqlite3_sys::{sqlite3, sqlite3_unlock_notify, SQLITE_OK};

use crate::SqliteError;

/// Blocks the thread until the connection that blocked `conn` ends its transaction.
///
/// Fails, without waiting, when SQLite refuses the registration: it does so when waiting
/// would deadlock. Nothing else ends the wait (see the module docs).
///
/// # Safety
///
/// `conn` must be an open connection handle that nothing else uses until this returns.
pub unsafe fn wait(conn: *mut sqlite3) -> Result<(), SqliteError> {
    let notify = Notify::new();

    // SAFETY: SQLite keeps the pointer to `notify` until it calls `unlock_notify_cb` with
    // it, perhaps from inside this call. `notify` lives on this frame, which cannot return
    // before that call: either registration fails (no callback is kept) or `notify.wait()`
    // blocks until the callback has fired it.
    if sqlite3_unlock_notify(
        conn,
        Some(unlock_notify_cb),
        &notify as *const Notify as *mut Notify as *mut _,
    ) != SQLITE_OK
    {
        return Err(SqliteError::new(conn));
    }

    notify.wait();

    Ok(())
}

/// Called by SQLite with the context pointers of every waiter it releases at once, each a
/// `Notify` that `wait` registered and is still blocked on.
unsafe extern "C" fn unlock_notify_cb(ptr: *mut *mut c_void, len: c_int) {
    let ptr = ptr as *mut &Notify;
    // A negative count never comes; it reads as none rather than panicking into C.
    let slice = slice::from_raw_parts(ptr, usize::try_from(len).unwrap_or(0));

    for notify in slice {
        notify.fire();
    }
}

/// A one-shot signal: `fire` sets the flag that `wait` blocks on.
struct Notify {
    mutex: Mutex<bool>,
    condvar: Condvar,
}

impl Notify {
    fn new() -> Self {
        Self {
            mutex: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    fn wait(&self) {
        // The guard is dropped at once: the flag is all this waits for.
        #[allow(let_underscore_lock)]
        let _ = self
            .condvar
            .wait_while(self.mutex.lock().unwrap(), |fired| !*fired)
            .unwrap();
    }

    fn fire(&self) {
        let mut lock = self.mutex.lock().unwrap();
        *lock = true;
        self.condvar.notify_one();
    }
}
