//! Values decoded by the type their column declares, with no Rust type
//! chosen by the caller.

use chrono::{NaiveDate, NaiveDateTime};
use uuid::Uuid;

use crate::collation::Collation;
use crate::decode::Decode;
use crate::error::BoxDynError;
use crate::protocol::text::{ColumnFlags, ColumnType, ExtendedType};
use crate::types::{Json, JsonValue, MySqlTime};
use crate::value::ValueRef;
use crate::MySqlValueRef;

/// A decoded MySQL or MariaDB value, with every digit of a `DECIMAL`, the
/// full range of an `UNSIGNED` integer and the sign of a `TIME`.
#[derive(Debug, Clone, PartialEq)]
pub enum MySqlDecodedValue {
    /// SQL NULL, or a zero date.
    Null,
    /// A `BOOLEAN`, which is a `TINYINT(1)`.
    Bool(bool),
    /// A signed integer.
    Integer(i64),
    /// An `UNSIGNED` integer or a `BIT` value.
    Unsigned(u64),
    /// A `FLOAT` or `DOUBLE`.
    Float(f64),
    /// A `DECIMAL`'s digits as the server sent them.
    Decimal(String),
    /// Text, including `ENUM` and `SET` values.
    Text(String),
    /// Binary data.
    Bytes(Vec<u8>),
    /// A JSON document: MySQL's `JSON`, or MariaDB's by its extended type.
    Json(JsonValue),
    /// MariaDB's `UUID`, or the bytes of a `BINARY(16)`.
    Uuid(Uuid),
    /// A `DATE`.
    Date(NaiveDate),
    /// A `TIME`: a signed duration of up to 838 hours, not a time of day.
    Time(MySqlTime),
    /// A `DATETIME` or `TIMESTAMP`, which carry no offset.
    DateTime(NaiveDateTime),
}

impl MySqlValueRef<'_> {
    /// Decode by the type the column declares, never by what the value
    /// looks like. A `GEOMETRY` value returns an error.
    pub fn decode_lossless(&self) -> Result<MySqlDecodedValue, BoxDynError> {
        use MySqlDecodedValue as V;
        if self.is_null() {
            return Ok(V::Null);
        }
        let ty = &self.type_info;
        match ty.extended {
            ExtendedType::Json => return Ok(V::Json(Json::<JsonValue>::decode(self.clone())?.0)),
            ExtendedType::Uuid => return Ok(V::Uuid(Uuid::decode(self.clone())?)),
            ExtendedType::None => {}
        }
        Ok(match ty.r#type {
            ColumnType::Tiny if ty.max_size == Some(1) => V::Bool(bool::decode(self.clone())?),
            ColumnType::Tiny
            | ColumnType::Short
            | ColumnType::Long
            | ColumnType::Int24
            | ColumnType::LongLong
            | ColumnType::Year
            | ColumnType::Bit => {
                let value = super::int::integer_decode(self.clone())?;
                if ty.flags.contains(ColumnFlags::UNSIGNED) || ty.r#type == ColumnType::Bit {
                    V::Unsigned(value.try_into()?)
                } else {
                    V::Integer(value.try_into()?)
                }
            }
            ColumnType::Float | ColumnType::Double => V::Float(f64::decode(self.clone())?),
            ColumnType::Decimal | ColumnType::NewDecimal => V::Decimal(self.as_str()?.to_owned()),
            ColumnType::Json => V::Json(Json::<JsonValue>::decode(self.clone())?.0),
            ColumnType::Date => V::Date(NaiveDate::decode(self.clone())?),
            ColumnType::Time => V::Time(MySqlTime::decode(self.clone())?),
            ColumnType::Datetime | ColumnType::Timestamp => {
                V::DateTime(NaiveDateTime::decode(self.clone())?)
            }
            // Text and binary data share these wire types; binary data is
            // the `binary` collation, as a `_bin` text collation is not.
            ColumnType::VarChar
            | ColumnType::VarString
            | ColumnType::String
            | ColumnType::TinyBlob
            | ColumnType::Blob
            | ColumnType::MediumBlob
            | ColumnType::LongBlob
            | ColumnType::Enum
            | ColumnType::Set => {
                if ty.collation != Collation::BINARY {
                    V::Text(self.as_str()?.to_owned())
                } else if ty.r#type == ColumnType::String && ty.max_size == Some(16) {
                    V::Uuid(Uuid::from_slice(self.as_bytes()?)?)
                } else {
                    V::Bytes(self.as_bytes()?.to_vec())
                }
            }
            ColumnType::Null | ColumnType::Geometry => {
                return Err(format!("cannot decode a {} value", self.type_info).into())
            }
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/decoded_tests.rs"]
mod tests;
