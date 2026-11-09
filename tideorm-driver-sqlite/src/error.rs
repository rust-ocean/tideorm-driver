use std::error::Error as StdError;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};
use std::os::raw::c_int;
use std::{borrow::Cow, str};

use libsqlite3_sys::{
    sqlite3, sqlite3_errmsg, sqlite3_errstr, sqlite3_extended_errcode, SQLITE_BUSY,
    SQLITE_CONSTRAINT_CHECK, SQLITE_CONSTRAINT_FOREIGNKEY, SQLITE_CONSTRAINT_NOTNULL,
    SQLITE_CONSTRAINT_PRIMARYKEY, SQLITE_CONSTRAINT_UNIQUE, SQLITE_ERROR, SQLITE_LOCKED,
    SQLITE_NOMEM,
};

pub(crate) use driver_core::error::*;

// Error Codes And Messages
// https://www.sqlite.org/c3ref/errcode.html

#[derive(Debug)]
pub struct SqliteError {
    code: c_int,
    message: Cow<'static, str>,
}

impl SqliteError {
    pub(crate) unsafe fn new(handle: *mut sqlite3) -> Self {
        Self::try_new(handle).expect("There should be an error")
    }

    pub(crate) unsafe fn try_new(handle: *mut sqlite3) -> Option<Self> {
        // returns the extended result code even when extended result codes are disabled
        let code: c_int = unsafe { sqlite3_extended_errcode(handle) };

        if code == 0 {
            return None;
        }

        // return English-language text that describes the error
        let message = unsafe {
            let msg = sqlite3_errmsg(handle);
            debug_assert!(!msg.is_null());

            String::from_utf8_lossy(CStr::from_ptr(msg).to_bytes()).into_owned()
        };

        Some(Self {
            code,
            message: message.into(),
        })
    }

    /// For errors during extension load, the error message is supplied via a separate pointer
    pub(crate) fn with_message(mut self, error_msg: String) -> Self {
        self.message = error_msg.into();
        self
    }

    pub(crate) fn from_code(code: c_int) -> Self {
        let message = unsafe {
            let errstr = sqlite3_errstr(code);

            if !errstr.is_null() {
                // SAFETY: `errstr` is guaranteed to be UTF-8
                // The lifetime of the string is "internally managed";
                // the implementation just selects from an array of static strings.
                // We copy to an owned buffer in case `libsqlite3` is dynamically loaded somehow.
                Cow::Owned(str::from_utf8_unchecked(CStr::from_ptr(errstr).to_bytes()).into())
            } else {
                Cow::Borrowed("<error message unavailable>")
            }
        };

        SqliteError { code, message }
    }

    pub(crate) fn generic(message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code: SQLITE_ERROR,
            message: message.into(),
        }
    }

    /// Return `SQLITE_NOMEM`.
    pub(crate) fn nomem() -> Self {
        Self::from_code(SQLITE_NOMEM)
    }
}

impl Display for SqliteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // We include the code as some produce ambiguous messages:
        // SQLITE_BUSY: "database is locked"
        // SQLITE_LOCKED: "database table is locked"
        // Sadly there's no function to get the string label back from an error code.
        write!(f, "(code: {}) {}", self.code, self.message)
    }
}

impl StdError for SqliteError {}

impl DatabaseError for SqliteError {
    #[inline]
    fn message(&self) -> &str {
        &self.message
    }

    /// The extended result code.
    #[inline]
    fn code(&self) -> Option<Cow<'_, str>> {
        Some(format!("{}", self.code).into())
    }

    #[doc(hidden)]
    fn as_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
        self
    }

    #[doc(hidden)]
    fn into_error(self: Box<Self>) -> Box<dyn StdError + Send + Sync + 'static> {
        self
    }

    fn kind(&self) -> ErrorKind {
        kind_of(self.code, &self.message)
    }
}

/// The kind of error an extended result code names. SQLite reports a missing
/// table or column and a syntax error all as the generic `SQLITE_ERROR`, so
/// its message tells them apart.
fn kind_of(code: c_int, message: &str) -> ErrorKind {
    match code {
        SQLITE_CONSTRAINT_UNIQUE | SQLITE_CONSTRAINT_PRIMARYKEY => ErrorKind::UniqueViolation,
        SQLITE_CONSTRAINT_FOREIGNKEY => ErrorKind::ForeignKeyViolation,
        SQLITE_CONSTRAINT_NOTNULL => ErrorKind::NotNullViolation,
        SQLITE_CONSTRAINT_CHECK => ErrorKind::CheckViolation,
        // A busy or locked database, under any of their extended codes: the
        // contention a retry can clear.
        code if matches!(code & 0xff, SQLITE_BUSY | SQLITE_LOCKED) => ErrorKind::LockNotAvailable,
        SQLITE_ERROR if message.starts_with("no such table") => ErrorKind::UndefinedTable,
        SQLITE_ERROR
            if message.starts_with("no such column") || message.contains("has no column named") =>
        {
            ErrorKind::UndefinedColumn
        }
        SQLITE_ERROR if message.contains("syntax error") => ErrorKind::SyntaxError,
        _ => ErrorKind::Other,
    }
}

#[cfg(test)]
#[path = "../tests/unit/error_tests.rs"]
mod tests;
