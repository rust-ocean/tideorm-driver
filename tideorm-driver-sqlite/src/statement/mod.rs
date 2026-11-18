use crate::column::ColumnIndex;
use crate::error::Error;
use crate::ext::ustr::UStr;
use crate::{Sqlite, SqliteArguments, SqliteColumn, SqliteTypeInfo};
use driver_core::sql_str::SqlStr;
use driver_core::{Either, HashMap};
use std::sync::Arc;

pub(crate) use driver_core::statement::*;

mod handle;

pub(super) mod unlock_notify;
mod r#virtual;

pub(crate) use handle::StatementHandle;
pub(crate) use r#virtual::VirtualStatement;

/// A prepared SQLite statement as `Executor::prepare` describes it: its SQL, how many
/// parameters it takes and the columns it returns.
///
/// SQLite reports parameters only by count, never by type.
#[derive(Debug, Clone)]
#[allow(clippy::rc_buffer)]
pub struct SqliteStatement {
    pub(crate) sql: SqlStr,
    pub(crate) parameters: usize,
    pub(crate) columns: Arc<Vec<SqliteColumn>>,
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
}

impl Statement for SqliteStatement {
    type Database = Sqlite;

    fn sql(&self) -> &SqlStr {
        &self.sql
    }

    fn parameters(&self) -> Option<Either<&[SqliteTypeInfo], usize>> {
        Some(Either::Right(self.parameters))
    }

    fn columns(&self) -> &[SqliteColumn] {
        &self.columns
    }

    impl_statement_query!(SqliteArguments);
}

impl ColumnIndex<SqliteStatement> for &'_ str {
    fn index(&self, statement: &SqliteStatement) -> Result<usize, Error> {
        driver_core::column::column_by_name(&statement.column_names, self)
    }
}
