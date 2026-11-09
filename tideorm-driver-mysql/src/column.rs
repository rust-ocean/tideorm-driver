use crate::error::Error;
use crate::ext::ustr::UStr;
use crate::HashMap;
use crate::{MySql, MySqlTypeInfo};
pub(crate) use driver_core::column::*;

#[derive(Debug, Clone)]
pub struct MySqlColumn {
    pub(crate) ordinal: usize,
    pub(crate) name: UStr,
    pub(crate) type_info: MySqlTypeInfo,

    pub(crate) origin: ColumnOrigin,
}

impl Column for MySqlColumn {
    type Database = MySql;

    fn ordinal(&self) -> usize {
        self.ordinal
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_info(&self) -> &MySqlTypeInfo {
        &self.type_info
    }

    fn origin(&self) -> ColumnOrigin {
        self.origin.clone()
    }
}

/// The ordinal of the column called `name`, looked up the same way for a row
/// and for a statement.
pub(crate) fn ordinal_by_name(names: &HashMap<UStr, usize>, name: &str) -> Result<usize, Error> {
    names
        .get(name)
        .copied()
        .ok_or_else(|| Error::ColumnNotFound(name.into()))
}
