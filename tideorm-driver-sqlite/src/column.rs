use crate::ext::ustr::UStr;
use crate::{Sqlite, SqliteTypeInfo};

pub(crate) use driver_core::column::*;

#[derive(Debug, Clone)]
pub struct SqliteColumn {
    pub(crate) name: UStr,
    pub(crate) ordinal: usize,
    pub(crate) type_info: SqliteTypeInfo,
    pub(crate) declared_type: Option<UStr>,

    pub(crate) origin: ColumnOrigin,
}

impl SqliteColumn {
    /// The type the column's table declares it with, as written (`JSON`,
    /// `VARCHAR(20)`), even one the driver has no type for; `None` for an
    /// expression.
    pub fn declared_type(&self) -> Option<&str> {
        self.declared_type.as_deref()
    }
}

impl Column for SqliteColumn {
    type Database = Sqlite;

    fn ordinal(&self) -> usize {
        self.ordinal
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_info(&self) -> &SqliteTypeInfo {
        &self.type_info
    }

    fn origin(&self) -> ColumnOrigin {
        self.origin.clone()
    }
}
