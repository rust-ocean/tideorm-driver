// The column metadata is shared with the statement that produced the row, hence the `Arc`s.
#![allow(clippy::rc_buffer)]

use std::sync::Arc;

use driver_core::column::ColumnIndex;
use driver_core::error::Error;
use driver_core::ext::ustr::UStr;
use driver_core::row::{debug_row, Row};
use driver_core::HashMap;

use crate::statement::{column_index, StatementHandle};
use crate::{Sqlite, SqliteColumn, SqliteValue, SqliteValueRef};

/// One row of a SQLite result: its own copy of every value, and the statement's column
/// list and name index.
pub struct SqliteRow {
    pub(crate) values: Box<[SqliteValue]>,
    pub(crate) columns: Arc<Vec<SqliteColumn>>,
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
}

// The values are copies (`sqlite3_value_dup`), so a row holds nothing of the statement it
// came from and stepping that statement again cannot touch it. `Row` requires `Sync`, but
// decoding can change a value's SQLite-side state (see `SqliteValue`), so one row's values
// must not be decoded from two threads at the same time.
unsafe impl Send for SqliteRow {}
unsafe impl Sync for SqliteRow {}

impl SqliteRow {
    /// Copies out the row `statement` currently stands on, after a step that returned
    /// `SQLITE_ROW`.
    pub(crate) fn current(
        statement: &StatementHandle,
        columns: &Arc<Vec<SqliteColumn>>,
        column_names: &Arc<HashMap<UStr, usize>>,
    ) -> Self {
        let values = (0..statement.column_count())
            .map(|i| {
                // SAFETY: `i` is a column of the row the statement stands on, for which
                // SQLite hands out a valid value that it owns until the next step; `dup`
                // copies it at once and frees the copy when it drops.
                unsafe {
                    SqliteValue::dup(
                        statement.column_value(i),
                        Some(columns[i].type_info.clone()),
                    )
                }
            })
            .collect();

        Self {
            values,
            columns: Arc::clone(columns),
            column_names: Arc::clone(column_names),
        }
    }
}

impl Row for SqliteRow {
    type Database = Sqlite;

    fn columns(&self) -> &[SqliteColumn] {
        &self.columns
    }

    fn try_get_raw<I>(&self, index: I) -> Result<SqliteValueRef<'_>, Error>
    where
        I: ColumnIndex<Self>,
    {
        let index = index.index(self)?;
        Ok(SqliteValueRef::value(&self.values[index]))
    }
}

impl ColumnIndex<SqliteRow> for &'_ str {
    fn index(&self, row: &SqliteRow) -> Result<usize, Error> {
        column_index(&row.column_names, self)
    }
}

impl std::fmt::Debug for SqliteRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_row(self, f)
    }
}
