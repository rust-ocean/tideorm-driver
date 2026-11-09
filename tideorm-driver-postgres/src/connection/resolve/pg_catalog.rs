//! The rows and column values read from `pg_catalog` to resolve types and columns.

use std::sync::Arc;

use driver_core::decode::Decode;
use driver_core::error::BoxDynError;
use driver_core::row::Row;
use driver_core::types::Type;

use crate::error::Error;
use crate::type_info::PgType;
use crate::types::Oid;
use crate::{PgRow, PgTypeInfo, PgValueRef, Postgres};

#[derive(Debug)]
pub(super) struct TypeResolverRow {
    pub(super) oid: Oid,
    // Most of the time, these are the same but not necessarily for arrays
    pub(super) pretty_name: String,
    pub(super) catalog_name: String,
    pub(super) original_name: Option<String>,
    pub(super) typtype: TypType,
    pub(super) typcategory: TypCategory,
    pub(super) typelem: Option<Oid>,
    pub(super) typbasetype: Option<Oid>,
    pub(super) rngsubtype: Option<Oid>,
    pub(super) enum_labels: Vec<String>,
    pub(super) record_attributes: Vec<(String, Oid)>,
}

impl TypeResolverRow {
    pub(super) fn from_row(row: &PgRow) -> Result<Self, Error> {
        Ok(Self {
            oid: row.try_get("oid")?,
            pretty_name: row.try_get("pretty_name")?,
            catalog_name: row.try_get("catalog_name")?,
            original_name: row.try_get("original_name")?,
            typtype: row.try_get("typtype")?,
            typcategory: row.try_get("typcategory")?,
            typelem: row.try_get("typelem")?,
            typbasetype: row.try_get("typbasetype")?,
            rngsubtype: row.try_get("rngsubtype")?,
            enum_labels: row.try_get("enum_labels")?,
            record_attributes: row.try_get("record_attributes")?,
        })
    }
}

#[derive(Debug)]
pub(super) struct ColumnResolverRow {
    pub(super) table_oid: Oid,
    pub(super) table_name: Arc<str>,
    pub(super) columns: Vec<(i16, Arc<str>)>,
}

impl ColumnResolverRow {
    pub(super) fn from_row(row: &PgRow) -> Result<Self, Error> {
        Ok(Self {
            table_oid: row.try_get("table_oid")?,
            table_name: row.try_get("table_name")?,
            columns: row.try_get("columns")?,
        })
    }
}

/// Describes the type of the `pg_type.typtype` column
///
/// See <https://www.postgresql.org/docs/13/catalog-pg-type.html>
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum TypType {
    Base,
    Composite,
    Domain,
    Enum,
    Pseudo,
    Range,
    Multirange,
}

impl TryFrom<i8> for TypType {
    type Error = String;

    fn try_from(t: i8) -> Result<Self, Self::Error> {
        let t = u8::try_from(t).map_err(|_| format!("unknown type code {t}"))?;

        let t = match t {
            b'b' => Self::Base,
            b'c' => Self::Composite,
            b'd' => Self::Domain,
            b'e' => Self::Enum,
            b'p' => Self::Pseudo,
            b'r' => Self::Range,
            b'm' => Self::Multirange,
            _ => return Err(format!("unknown type code {t}")),
        };
        Ok(t)
    }
}

impl<'r> Decode<'r, Postgres> for TypType {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(i8::decode(value)?.try_into()?)
    }
}

impl Type<Postgres> for TypType {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo(PgType::Char)
    }
}

/// Describes the type of the `pg_type.typcategory` column
///
/// See <https://www.postgresql.org/docs/13/catalog-pg-type.html#CATALOG-TYPCATEGORY-TABLE>
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum TypCategory {
    Array,
    Boolean,
    Composite,
    DateTime,
    Enum,
    Geometric,
    Network,
    Numeric,
    Pseudo,
    Range,
    String,
    Timespan,
    User,
    BitString,
    Unknown,
}

impl TryFrom<i8> for TypCategory {
    type Error = String;

    fn try_from(c: i8) -> Result<Self, Self::Error> {
        let c = u8::try_from(c).map_err(|_| format!("invalid category code {c}"))?;

        let c = match c {
            b'A' => Self::Array,
            b'B' => Self::Boolean,
            b'C' => Self::Composite,
            b'D' => Self::DateTime,
            b'E' => Self::Enum,
            b'G' => Self::Geometric,
            b'I' => Self::Network,
            b'N' => Self::Numeric,
            b'P' => Self::Pseudo,
            b'R' => Self::Range,
            b'S' => Self::String,
            b'T' => Self::Timespan,
            b'U' => Self::User,
            b'V' => Self::BitString,
            b'X' => Self::Unknown,
            _ => return Err(format!("invalid category code {c}")),
        };
        Ok(c)
    }
}

impl<'r> Decode<'r, Postgres> for TypCategory {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(i8::decode(value)?.try_into()?)
    }
}

impl Type<Postgres> for TypCategory {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo(PgType::Char)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/type_resolver_tests.rs"]
mod tests;
