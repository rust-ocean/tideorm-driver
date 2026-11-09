use crate::column::ColumnIndex;
use crate::error::Error;
use crate::message::DataRow;
use crate::statement::PgStatementMetadata;
use crate::value::PgValueFormat;
use crate::{PgColumn, PgValueRef, Postgres};
use driver_core::row::debug_row;
pub(crate) use driver_core::row::Row;
use std::sync::Arc;

/// One row of a result: the server's `DataRow`, read through its statement's columns.
pub struct PgRow {
    pub(crate) data: DataRow,
    /// Every value of the row is in this format: binary from a prepared statement, text from
    /// an unprepared one.
    pub(crate) format: PgValueFormat,
    pub(crate) metadata: Arc<PgStatementMetadata>,
}

impl Row for PgRow {
    type Database = Postgres;

    fn columns(&self) -> &[PgColumn] {
        &self.metadata.columns
    }

    fn try_get_raw<I>(&self, index: I) -> Result<PgValueRef<'_>, Error>
    where
        I: ColumnIndex<Self>,
    {
        let index = index.index(self)?;

        Ok(PgValueRef {
            format: self.format,
            row: Some(&self.data.storage),
            type_info: self.metadata.columns[index].type_info.clone(),
            value: self.data.get(index),
        })
    }
}

impl ColumnIndex<PgRow> for &'_ str {
    fn index(&self, row: &PgRow) -> Result<usize, Error> {
        row.metadata
            .column_names
            .get(*self)
            .copied()
            .ok_or_else(|| Error::ColumnNotFound((*self).into()))
    }
}

impl std::fmt::Debug for PgRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_row(self, f)
    }
}
