//! Values decoded by what SQLite stores and the type their column declares,
//! with no Rust type chosen by the caller.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use super::chrono::{
    decode_datetime_from_float, decode_datetime_from_int, decode_datetime_from_text,
    decode_time_from_text,
};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::value::ValueRef;
use crate::SqliteValueRef;

/// A decoded SQLite value.
#[derive(Debug, Clone, PartialEq)]
pub enum SqliteDecodedValue {
    /// SQL NULL.
    Null,
    /// An integer in a column declared `BOOLEAN`.
    Bool(bool),
    /// An integer.
    Integer(i64),
    /// A real, as SQLite also stores a `DECIMAL` column's values.
    Real(f64),
    /// Text, however it looks.
    Text(String),
    /// A blob.
    Blob(Vec<u8>),
    /// Text in a column declared `DATE` that holds a date.
    Date(NaiveDate),
    /// Text in a column declared `TIME` that holds a time of day.
    Time(NaiveTime),
    /// A value in a column declared `DATETIME` or `TIMESTAMP` that holds a
    /// timestamp: text, Unix seconds or a Julian day.
    DateTime(NaiveDateTime),
}

impl SqliteValueRef<'_> {
    /// Decode the value SQLite stores, which is all an expression or a column
    /// declared as a type SQLite does not name (`DECIMAL`, `JSON`) has. A
    /// value of the form a declared boolean, date, time or timestamp column
    /// gives it is read as one; any other comes back as stored.
    pub fn decode_lossless(&self) -> Result<SqliteDecodedValue, BoxDynError> {
        use SqliteDecodedValue as V;
        if self.is_null() {
            return Ok(V::Null);
        }
        let declared = self.column_type().map(|ty| ty.0);
        // Not null, so this is the storage class.
        Ok(match self.type_info().0 {
            DataType::Integer => {
                let value = self.int64()?;
                match declared {
                    Some(DataType::Bool) => V::Bool(value != 0),
                    Some(DataType::Datetime) => decode_datetime_from_int(value)
                        .map_or(V::Integer(value), |at| V::DateTime(at.naive_local())),
                    _ => V::Integer(value),
                }
            }
            DataType::Float => {
                let value = self.double()?;
                match declared {
                    Some(DataType::Datetime) => decode_datetime_from_float(value)
                        .map_or(V::Real(value), |at| V::DateTime(at.naive_local())),
                    _ => V::Real(value),
                }
            }
            DataType::Text => {
                let text = self.text_owned()?;
                let moment = match declared {
                    Some(DataType::Date) => {
                        NaiveDate::parse_from_str(&text, "%F").ok().map(V::Date)
                    }
                    Some(DataType::Time) => decode_time_from_text(&text).map(V::Time),
                    Some(DataType::Datetime) => {
                        decode_datetime_from_text(&text).map(|at| V::DateTime(at.naive_local()))
                    }
                    _ => None,
                };
                moment.unwrap_or(V::Text(text))
            }
            _ => V::Blob(self.blob_owned()),
        })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/decoded_tests.rs"]
mod tests;
