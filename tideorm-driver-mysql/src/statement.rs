use super::MySqlColumn;
use crate::column::{ordinal_by_name, ColumnIndex};
use crate::error::Error;
use crate::ext::ustr::UStr;
use crate::HashMap;
use crate::{MySql, MySqlArguments, MySqlTypeInfo};
use driver_core::sql_str::SqlStr;
use either::Either;
use std::sync::Arc;

pub(crate) use driver_core::statement::*;

/// A statement the server has prepared: its SQL, how many parameters it takes
/// and the columns it returns.
#[derive(Debug, Clone)]
pub struct MySqlStatement {
    pub(crate) sql: SqlStr,
    pub(crate) metadata: MySqlStatementMetadata,
}

/// What a prepare reported, kept with each cached statement. Its column
/// lists are shared with the rows the statement returns.
#[derive(Debug, Default, Clone)]
pub(crate) struct MySqlStatementMetadata {
    pub(crate) columns: Arc<Vec<MySqlColumn>>,
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
    pub(crate) parameters: usize,
}

impl Statement for MySqlStatement {
    type Database = MySql;

    fn sql(&self) -> &SqlStr {
        &self.sql
    }

    fn parameters(&self) -> Option<Either<&[MySqlTypeInfo], usize>> {
        // A count only: the server's parameter definitions say nothing useful
        // about the types it expects.
        Some(Either::Right(self.metadata.parameters))
    }

    fn columns(&self) -> &[MySqlColumn] {
        &self.metadata.columns
    }

    impl_statement_query!(MySqlArguments);
}

impl ColumnIndex<MySqlStatement> for &'_ str {
    fn index(&self, statement: &MySqlStatement) -> Result<usize, Error> {
        ordinal_by_name(&statement.metadata.column_names, self)
    }
}
