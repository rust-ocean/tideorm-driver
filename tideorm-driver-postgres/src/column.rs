use crate::ext::ustr::UStr;
use crate::{PgTypeInfo, Postgres};

use driver_core::column::ColumnOrigin;
pub(crate) use driver_core::column::{Column, ColumnIndex};

/// A column of a statement's result, as the server's `RowDescription` describes it.
#[derive(Debug, Clone)]
pub struct PgColumn {
    pub(crate) ordinal: usize,
    pub(crate) name: UStr,
    pub(crate) type_info: PgTypeInfo,

    /// The table and column it reads, if the catalog could name them; else an expression or
    /// unknown.
    pub(crate) origin: ColumnOrigin,

    pub(crate) relation_id: Option<crate::types::Oid>,
    pub(crate) relation_attribute_no: Option<i16>,
}

impl PgColumn {
    /// The OID of the table the column is read from (`pg_attribute.attrelid`), or `None` when it
    /// is computed by an expression.
    ///
    /// <https://www.postgresql.org/docs/current/catalog-pg-attribute.html>
    pub fn relation_id(&self) -> Option<crate::types::Oid> {
        self.relation_id
    }

    /// The column's number in that table (`pg_attribute.attnum`, counted from 1), or `None` when
    /// it is computed by an expression.
    ///
    /// <https://www.postgresql.org/docs/current/catalog-pg-attribute.html>
    pub fn relation_attribute_no(&self) -> Option<i16> {
        self.relation_attribute_no
    }
}

impl Column for PgColumn {
    type Database = Postgres;

    fn ordinal(&self) -> usize {
        self.ordinal
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_info(&self) -> &PgTypeInfo {
        &self.type_info
    }

    fn origin(&self) -> ColumnOrigin {
        self.origin.clone()
    }
}
