//! The settings whose `PRAGMA` takes one of a fixed set of keywords; the connect options
//! send the chosen keyword when a connection opens.
//!
//! The pragmas are documented at <https://www.sqlite.org/pragma.html>.

use crate::error::Error;
use std::str::FromStr;

/// Declares a pragma's enum: each variant with the keyword it sends, `as_str` giving
/// that keyword, and a `FromStr` that reads any keyword in any case, as SQLite does,
/// and names the pragma when it meets an unknown one.
macro_rules! pragma_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident for $pragma:literal {
            $($(#[$variant_meta:meta])* $variant:ident = $keyword:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)+
        }

        impl $name {
            pub(crate) fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $keyword,)+
                }
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self, Error> {
                $(if s.eq_ignore_ascii_case($keyword) {
                    return Ok(Self::$variant);
                })+

                Err(Error::Configuration(
                    format!("unknown value {s:?} for `{}`", $pragma).into(),
                ))
            }
        }
    };
}

pragma_enum! {
    /// Whether SQLite gives freed pages back to the file system: never, at every commit,
    /// or when `PRAGMA incremental_vacuum` asks.
    ///
    /// See [`PRAGMA auto_vacuum`](https://www.sqlite.org/pragma.html#pragma_auto_vacuum).
    pub enum SqliteAutoVacuum for "auto_vacuum" {
        #[default]
        None = "NONE",
        Full = "FULL",
        Incremental = "INCREMENTAL",
    }
}

pragma_enum! {
    /// How SQLite keeps its rollback journal, or whether it writes ahead to a log
    /// (`Wal`, the default here) instead.
    ///
    /// See [`PRAGMA journal_mode`](https://www.sqlite.org/pragma.html#pragma_journal_mode).
    pub enum SqliteJournalMode for "journal_mode" {
        Delete = "DELETE",
        Truncate = "TRUNCATE",
        Persist = "PERSIST",
        Memory = "MEMORY",
        #[default]
        Wal = "WAL",
        Off = "OFF",
    }
}

pragma_enum! {
    /// Whether a connection releases its file locks after each transaction (`Normal`) or
    /// holds every lock it takes for as long as it stays in this mode (`Exclusive`).
    ///
    /// See [`PRAGMA locking_mode`](https://www.sqlite.org/pragma.html#pragma_locking_mode).
    pub enum SqliteLockingMode for "locking_mode" {
        #[default]
        Normal = "NORMAL",
        Exclusive = "EXCLUSIVE",
    }
}

pragma_enum! {
    /// How often SQLite waits for writes to reach the disk, trading durability for
    /// speed.
    ///
    /// See [`PRAGMA synchronous`](https://www.sqlite.org/pragma.html#pragma_synchronous).
    pub enum SqliteSynchronous for "synchronous" {
        Off = "OFF",
        Normal = "NORMAL",
        #[default]
        Full = "FULL",
        Extra = "EXTRA",
    }
}

#[cfg(test)]
#[path = "../../tests/unit/options_pragmas_tests.rs"]
mod tests;
