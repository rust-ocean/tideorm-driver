use super::{PgColumn, PgTypeInfo};
use crate::column::ColumnIndex;
use crate::error::Error;
use crate::ext::ustr::UStr;
use crate::{PgArguments, Postgres};
use std::sync::Arc;

use driver_core::sql_str::SqlStr;
pub(crate) use driver_core::statement::Statement;
use driver_core::{Either, HashMap};

#[derive(Debug, Clone)]
pub struct PgStatement {
    pub(crate) sql: SqlStr,
    pub(crate) metadata: Arc<PgStatementMetadata>,
}

#[derive(Debug, Default)]
pub(crate) struct PgStatementMetadata {
    pub(crate) columns: Vec<PgColumn>,
    // Avoid a deep copy when this map is shared with statement metadata.
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
    pub(crate) parameters: Vec<PgTypeInfo>,
}

impl Statement for PgStatement {
    type Database = Postgres;

    fn sql(&self) -> &SqlStr {
        &self.sql
    }

    fn parameters(&self) -> Option<Either<&[PgTypeInfo], usize>> {
        Some(Either::Left(&self.metadata.parameters))
    }

    fn columns(&self) -> &[PgColumn] {
        &self.metadata.columns
    }

    impl_statement_query!(PgArguments);
}

impl ColumnIndex<PgStatement> for &'_ str {
    fn index(&self, statement: &PgStatement) -> Result<usize, Error> {
        statement
            .metadata
            .column_names
            .get(*self)
            .ok_or_else(|| Error::ColumnNotFound((*self).into()))
            .copied()
    }
}
