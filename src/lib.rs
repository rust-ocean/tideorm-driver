//! Database drivers for TideORM: connections, pools, transactions, statements
//! and the conversion of Rust values to and from each database's types.
//!
//! Each database is a feature (`postgres`, `mysql`, `sqlite`) backed by its own
//! crate; `runtime-tokio` or `runtime-async-std` selects the async runtime and
//! `tls-rustls` enables TLS connections.

pub use driver_core::acquire::Acquire;
pub use driver_core::arguments::{Arguments, IntoArguments};
pub use driver_core::column::Column;
pub use driver_core::column::ColumnIndex;
pub use driver_core::column::ColumnOrigin;
pub use driver_core::connection::{ConnectOptions, Connection};
pub use driver_core::database::{self, Database};
pub use driver_core::executor::{Execute, Executor};
pub use driver_core::pool::{self, Pool};
#[doc(hidden)]
pub use driver_core::query::query_with_result as __query_with_result;
pub use driver_core::query::{query, query_with};
#[doc(hidden)]
pub use driver_core::row::Row;
pub use driver_core::sql_str::{AssertSqlSafe, SqlSafeStr, SqlStr};
pub use driver_core::statement::Statement;
pub use driver_core::transaction::Transaction;
pub use driver_core::type_info::TypeInfo;
pub use driver_core::types::Type;
pub use driver_core::value::{Value, ValueRef};
pub use driver_core::Either;

#[doc(inline)]
pub use driver_core::error::{self, Error, Result};

#[cfg(feature = "mysql")]
#[doc(inline)]
pub use driver_mysql::{
    self as mysql, MySql, MySqlConnection, MySqlExecutor, MySqlPool, MySqlTransaction,
};

#[cfg(feature = "postgres")]
#[doc(inline)]
pub use driver_postgres::{
    self as postgres, PgConnection, PgExecutor, PgPool, PgTransaction, Postgres,
};

#[cfg(feature = "sqlite")]
#[doc(inline)]
pub use driver_sqlite::{
    self as sqlite, Sqlite, SqliteConnection, SqliteExecutor, SqlitePool, SqliteTransaction,
};

#[doc(hidden)]
pub use driver_core::rt::test_block_on;

#[doc(hidden)]
pub use driver_core::rt as __rt;

/// Conversions between Rust and SQL types.
///
/// To see how each SQL type maps to a Rust type, see the corresponding `types` module for each
/// database:
///
///  * Postgres: [postgres::types]
///  * MySQL: [mysql::types]
///  * SQLite: [sqlite::types]
///
/// Any external types that have had [`Type`] implemented for, are re-exported in this module
/// for convenience as downstream users need to use a compatible version of the external crate
/// to take advantage of the implementation.
///
/// [`Type`]: types::Type
pub mod types {
    pub use driver_core::types::*;
}

/// Provides [`Encode`] for encoding values for the database.
pub mod encode {
    pub use driver_core::encode::{Encode, IsNull};
}

pub use self::encode::Encode;

/// Provides [`Decode`] for decoding values from the database.
pub mod decode {
    pub use driver_core::decode::Decode;
}

pub use self::decode::Decode;

/// Types and traits for the `query` family of functions.
pub mod query {
    pub use driver_core::query::{Map, Query};
}

/// Convenience re-export of common traits.
pub mod prelude {
    pub use super::Acquire;
    pub use super::ConnectOptions;
    pub use super::Connection;
    pub use super::Decode;
    pub use super::Encode;
    pub use super::Executor;
    pub use super::IntoArguments;
    pub use super::Row;
    pub use super::Statement;
    pub use super::Type;
}
