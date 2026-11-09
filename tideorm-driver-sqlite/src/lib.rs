//! **SQLite** database driver.
//!
//! This driver always builds SQLite 3 from the source bundled by `libsqlite3-sys` and
//! statically links it into the final binary. Building it requires a C toolchain; see
//! [the `rusqlite` build notes][rusqlite-readme-building] for details.
//!
//! Only one version of `libsqlite3-sys` may appear in a project's dependency tree.
//! If another crate also depends on it, their required versions must be compatible.
//!
//! [rusqlite-readme-building]: https://github.com/rusqlite/rusqlite?tab=readme-ov-file#notes-on-building-rusqlite-and-libsqlite3-sys

// SQLite is a C library. All interactions require FFI which is unsafe.
// All unsafe blocks should have comments pointing to SQLite docs and ensuring that we maintain
// invariants.
#![allow(unsafe_code)]

#[macro_use]
extern crate driver_core;

pub use arguments::{SqliteArgumentValue, SqliteArguments, SqliteArgumentsBuffer};
pub use column::SqliteColumn;
pub use connection::deserialize::SqliteOwnedBuf;
pub use connection::{LockedSqliteHandle, SqliteConnection, SqliteOperation, UpdateHookResult};
pub use database::Sqlite;
pub use error::SqliteError;
pub use options::{
    SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode, SqliteLockingMode, SqliteSynchronous,
};
pub use query_result::SqliteQueryResult;
pub use row::SqliteRow;
pub use statement::SqliteStatement;
pub use transaction::SqliteTransactionManager;
pub use type_info::SqliteTypeInfo;
pub use value::{SqliteValue, SqliteValueRef};

pub(crate) use driver_core::driver_prelude::*;

use driver_core::executor::Executor;

mod arguments;
mod column;
mod connection;
mod database;
mod error;
mod options;
mod query_result;
mod row;
mod statement;
mod transaction;
mod type_info;
pub mod types;
mod value;

/// An alias for [`Pool`][crate::pool::Pool], specialized for SQLite.
pub type SqlitePool = crate::pool::Pool<Sqlite>;

/// An alias for [`PoolOptions`][crate::pool::PoolOptions], specialized for SQLite.
pub type SqlitePoolOptions = crate::pool::PoolOptions<Sqlite>;

/// An alias for [`Executor<'_, Database = Sqlite>`][Executor].
pub trait SqliteExecutor<'c>: Executor<'c, Database = Sqlite> {}
impl<'c, T: Executor<'c, Database = Sqlite>> SqliteExecutor<'c> for T {}

/// An alias for [`Transaction`][driver_core::transaction::Transaction], specialized for SQLite.
pub type SqliteTransaction<'c> = driver_core::transaction::Transaction<'c, Sqlite>;

// NOTE: required due to the lack of lazy normalization
impl_into_arguments_for_arguments!(SqliteArguments);
impl_column_index_for_row!(SqliteRow);
impl_column_index_for_statement!(SqliteStatement);
impl_acquire!(Sqlite, SqliteConnection);

// required because some databases have a different handling of NULL
impl_encode_for_option!(Sqlite);
