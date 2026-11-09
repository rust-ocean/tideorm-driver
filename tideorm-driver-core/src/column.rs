//! Result columns: what a backend reports about each, and looking one up by name or
//! position.

use crate::database::Database;
use crate::error::Error;

use std::fmt::Debug;
use std::sync::Arc;

/// One column of a statement's result, as the backend describes it.
pub trait Column: 'static + Send + Sync + Debug {
    type Database: Database<Column = Self>;

    /// The column's position in the row, counting from 0. Unlike the name, it tells
    /// apart two columns that share a name.
    fn ordinal(&self) -> usize;

    /// The column's name, or its alias.
    ///
    /// For an expression with no alias, the server picks the name, and it may differ
    /// between server versions.
    fn name(&self) -> &str;

    /// The column's type as the backend reports it.
    fn type_info(&self) -> &<Self::Database as Database>::TypeInfo;

    /// The table column this result column reads, when the backend can tell.
    ///
    /// [`ColumnOrigin::Expression`] means the column is computed, or its table could not
    /// be found; [`ColumnOrigin::Unknown`], the default, that the backend has no such
    /// information. The value is returned owned so a backend can build it on demand.
    fn origin(&self) -> ColumnOrigin {
        ColumnOrigin::Unknown
    }
}

/// The table column a result column was read from.
#[derive(Debug, Clone)]
pub struct TableColumn {
    /// The table's name, schema-qualified where the backend reports a schema.
    pub table: Arc<str>,
    /// The column's name in that table, before any alias.
    pub name: Arc<str>,
}

/// What is known about where a result column comes from.
#[derive(Debug, Clone, Default)]
pub enum ColumnOrigin {
    /// A column of a table, named here.
    Table(TableColumn),
    /// An expression, or a column whose table could not be determined.
    Expression,
    /// Nothing is known: the backend keeps no origins, could not read its catalog
    /// (for one, while a query was still streaming), or does not implement
    /// [`Column::origin()`].
    #[default]
    Unknown,
}

impl ColumnOrigin {
    /// The table column, when the origin is one.
    pub fn table_column(&self) -> Option<&TableColumn> {
        if let Self::Table(table_column) = self {
            Some(table_column)
        } else {
            None
        }
    }
}

/// A key that picks one column of a [`Row`] or [`Statement`]: a `usize` position, or a
/// `&str` name. [`Row::get`] and [`Row::try_get`] take any `ColumnIndex`.
///
/// A statement and the rows it returns can disagree on positions, for instance when the
/// schema changed between preparing and running it, or when the backend reported no
/// columns at prepare time.
///
/// [`Row`]: crate::row::Row
/// [`Statement`]: crate::statement::Statement
/// [`Row::get`]: crate::row::Row::get
/// [`Row::try_get`]: crate::row::Row::try_get
pub trait ColumnIndex<T: ?Sized>: Debug {
    /// The position this key names in `container`, or [`Error::ColumnIndexOutOfBounds`]
    /// or [`Error::ColumnNotFound`] when there is none.
    fn index(&self, container: &T) -> Result<usize, Error>;
}

impl<T: ?Sized, I: ColumnIndex<T> + ?Sized> ColumnIndex<T> for &'_ I {
    #[inline]
    fn index(&self, row: &T) -> Result<usize, Error> {
        (**self).index(row)
    }
}

/// Implements [`ColumnIndex`] by position (`usize`) for a backend's row type, checked
/// against the row's length.
#[macro_export]
macro_rules! impl_column_index_for_row {
    ($R:ident) => {
        impl $crate::column::ColumnIndex<$R> for usize {
            fn index(&self, row: &$R) -> Result<usize, $crate::error::Error> {
                let len = $crate::row::Row::len(row);

                if *self >= len {
                    return Err($crate::error::Error::ColumnIndexOutOfBounds { len, index: *self });
                }

                Ok(*self)
            }
        }
    };
}

/// Implements [`ColumnIndex`] by position (`usize`) for a backend's statement type,
/// checked against the statement's column count.
#[macro_export]
macro_rules! impl_column_index_for_statement {
    ($S:ident) => {
        impl $crate::column::ColumnIndex<$S> for usize {
            fn index(&self, statement: &$S) -> Result<usize, $crate::error::Error> {
                let len = $crate::statement::Statement::columns(statement).len();

                if *self >= len {
                    return Err($crate::error::Error::ColumnIndexOutOfBounds { len, index: *self });
                }

                Ok(*self)
            }
        }
    };
}
