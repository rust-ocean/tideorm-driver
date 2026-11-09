use std::sync::Arc;

pub(crate) use driver_core::row::*;

use crate::column::{ordinal_by_name, ColumnIndex};
use crate::error::Error;
use crate::ext::ustr::UStr;
use crate::HashMap;
use crate::{protocol, MySql, MySqlColumn, MySqlValueFormat, MySqlValueRef};

/// Implementation of [`Row`] for MySQL: one row of a result, in the format
/// its statement was run with.
pub struct MySqlRow {
    pub(crate) row: protocol::Row,
    pub(crate) format: MySqlValueFormat,
    /// Shared by every row of one result.
    pub(crate) columns: Arc<Vec<MySqlColumn>>,
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
}

impl Row for MySqlRow {
    type Database = MySql;

    fn columns(&self) -> &[MySqlColumn] {
        &self.columns
    }

    fn try_get_raw<I>(&self, index: I) -> Result<MySqlValueRef<'_>, Error>
    where
        I: ColumnIndex<Self>,
    {
        let index = index.index(self)?;
        let column = &self.columns[index];
        let value = self.row.get(index);

        Ok(MySqlValueRef {
            format: self.format,
            row: Some(&self.row.storage),
            type_info: column.type_info.clone(),
            value,
        })
    }
}

impl ColumnIndex<MySqlRow> for &'_ str {
    fn index(&self, row: &MySqlRow) -> Result<usize, Error> {
        ordinal_by_name(&row.column_names, self)
    }
}

impl std::fmt::Debug for MySqlRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_row(self, f)
    }
}
