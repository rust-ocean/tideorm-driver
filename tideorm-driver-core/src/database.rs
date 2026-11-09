//! Traits to represent a database driver.
//!
//! TideORM provides drivers for MySQL, MariaDB, PostgreSQL, and SQLite. Applications
//! select a database driver at compile time through the corresponding front-crate feature.

use std::fmt::Debug;

use crate::arguments::Arguments;
use crate::column::Column;
use crate::connection::Connection;
use crate::row::Row;

use crate::statement::Statement;
use crate::transaction::TransactionManager;
use crate::type_info::TypeInfo;
use crate::value::{Value, ValueRef};

/// A database driver.
///
/// This trait encapsulates a complete set of traits that implement a driver for a
/// specific database (e.g., MySQL, PostgreSQL).
pub trait Database: 'static + Sized + Send + Debug {
    /// The concrete `Connection` implementation for this database.
    type Connection: Connection<Database = Self>;

    /// The concrete `TransactionManager` implementation for this database.
    type TransactionManager: TransactionManager<Database = Self>;

    /// The concrete `Row` implementation for this database.
    type Row: Row<Database = Self>;

    /// The concrete `QueryResult` implementation for this database.
    type QueryResult: QueryResult;

    /// The concrete `Column` implementation for this database.
    type Column: Column<Database = Self>;

    /// The concrete `TypeInfo` implementation for this database.
    type TypeInfo: TypeInfo;

    /// The concrete type used to hold an owned copy of the not-yet-decoded value that was
    /// received from the database.
    type Value: Value<Database = Self> + 'static;
    /// The concrete type used to hold a reference to the not-yet-decoded value that has just been
    /// received from the database.
    type ValueRef<'r>: ValueRef<'r, Database = Self>;

    /// The concrete `Arguments` implementation for this database.
    type Arguments: Arguments<Database = Self>;
    /// The concrete type used as a buffer for arguments while encoding.
    type ArgumentBuffer;

    /// The concrete `Statement` implementation for this database.
    type Statement: Statement<Database = Self>;

    /// The display name for this database driver.
    const NAME: &'static str;

    /// Maximum bind parameters in one prepared statement for this driver.
    ///
    /// This is the protocol or bundled database limit, not a recommended batch
    /// size. A server or connection configured with a lower limit can still
    /// reject a statement that fits it.
    const MAX_BIND_PARAMETERS: usize;

    /// The schemes for database URLs that should match this driver.
    const URL_SCHEMES: &'static [&'static str];
}

/// The outcome of executing statements, including outcomes combined by [`Extend`].
pub trait QueryResult: 'static + Sized + Send + Sync + Default + Extend<Self> {
    /// Number of rows affected, using the database's own counting semantics.
    fn rows_affected(&self) -> u64;

    /// The last insert identifier reported by the database, or `None` when
    /// the protocol does not report one (as on PostgreSQL).
    ///
    /// The signed representation preserves both SQLite's negative rowids and
    /// MySQL's full `u64` range. Zero remains a reported value, not `None`.
    /// Combining results retains the final result's identifier.
    fn last_insert_id(&self) -> Option<i128>;
}

/// A [`Database`] that maintains a client-side cache of prepared statements.
pub trait HasStatementCache {}
