//! Values that do not require a caller to select a bounded Rust representation.

use crate::error::BoxDynError;
use crate::type_info::{PgType, PgTypeKind};
use crate::types::chrono::{time_from_micros, MICROS_PER_DAY};
use crate::types::{PgMoney, PgUnrepresentableMoment};
use crate::{PgTypeInfo, PgValueFormat, PgValueRef};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use std::borrow::Cow;
use temporal::{
    date, date_text, inet_text, interval_text, timestamp, timestamp_text, timetz_text, Moment,
};
use wire::fixed;

mod array;
mod numeric;
mod range;
mod temporal;
mod wire;

/// A decoded PostgreSQL value, with exact numerics and special temporal values.
/// No JSON representation is imposed on the caller. Unrecognized binary types
/// are accepted only when their payload is printable UTF-8, as for enum labels.
#[derive(Debug, Clone, PartialEq)]
pub enum PgDecodedValue {
    /// SQL NULL.
    Null,
    /// A boolean.
    Bool(bool),
    /// A signed integer.
    Integer(i64),
    /// A floating point value, including non-finite values.
    Float(f64),
    /// Exact decimal digits, or a numeric special value.
    Numeric(String),
    /// Text, including the canonical representation of intervals and ranges.
    Text(String),
    /// An infinite date or timestamp, or the end-of-day time.
    SpecialMoment(&'static str),
    /// A byte string.
    Bytes(Vec<u8>),
    /// A JSON document.
    Json(serde_json::Value),
    /// A finite date.
    Date(NaiveDate),
    /// A finite time of day.
    Time(NaiveTime),
    /// A finite timestamp without time zone.
    Timestamp(NaiveDateTime),
    /// A finite timestamp with time zone, which PostgreSQL sends in UTC.
    TimestampWithTimeZone(DateTime<Utc>),
    /// A monetary amount in the server's smallest monetary unit, which
    /// [`PgMoney::to_decimal`] turns into an amount given the locale's
    /// fraction digits.
    Money(PgMoney),
    /// An array's shape and its elements, NULLs included.
    Array(PgDecodedArray),
}

/// The shape and contents of a PostgreSQL array.
#[derive(Debug, Clone, PartialEq)]
pub struct PgDecodedArray {
    /// The length of each dimension, outermost first; none for an empty array.
    pub dimensions: Vec<usize>,
    /// Elements in row-major order, retaining SQL NULLs.
    pub elements: Vec<PgDecodedValue>,
}

impl PgValueRef<'_> {
    /// Decode without narrowing numerics, dropping an array's NULLs or
    /// refusing an infinite date or `24:00:00`.
    /// Text-format results are returned verbatim; unknown binary representations
    /// that are not printable UTF-8 return an error.
    pub fn decode_lossless(&self) -> Result<PgDecodedValue, BoxDynError> {
        let Some(bytes) = self.value else {
            return Ok(PgDecodedValue::Null);
        };
        if self.format == PgValueFormat::Text {
            return Ok(PgDecodedValue::Text(self.as_str()?.to_owned()));
        }
        decode_type(&self.type_info, bytes)
            .ok_or_else(|| format!("cannot decode binary {} value", self.type_info).into())
    }

    /// The date, time or timestamp this holds when no Rust date or time type
    /// can hold it: `infinity`, `-infinity` or `TIME '24:00:00'`.
    pub fn unrepresentable_moment(&self) -> Option<PgUnrepresentableMoment> {
        let (bytes, type_name) = (self.value?, self.type_info.moment_name()?);
        let value = match (type_name, self.format) {
            ("TIME", PgValueFormat::Text) => bytes.starts_with(b"24:").then_some("24:00:00")?,
            ("TIME", PgValueFormat::Binary) => {
                (i64::from_be_bytes(fixed(bytes)?) == MICROS_PER_DAY).then_some("24:00:00")?
            }
            (_, PgValueFormat::Text) => match bytes {
                b"infinity" => "infinity",
                b"-infinity" => "-infinity",
                _ => return None,
            },
            ("DATE", PgValueFormat::Binary) => match i32::from_be_bytes(fixed(bytes)?) {
                i32::MAX => "infinity",
                i32::MIN => "-infinity",
                _ => return None,
            },
            (_, PgValueFormat::Binary) => match i64::from_be_bytes(fixed(bytes)?) {
                i64::MAX => "infinity",
                i64::MIN => "-infinity",
                _ => return None,
            },
        };
        Some(PgUnrepresentableMoment { type_name, value })
    }
}

/// The type's name in upper case, as `binary_value` matches it: a built-in
/// type's display name, or an extension type's catalog name (`citext`).
fn type_name(ty: &PgTypeInfo) -> Cow<'_, str> {
    match &ty.0 {
        PgType::Custom(custom) => Cow::Owned(custom.name.to_ascii_uppercase()),
        _ => Cow::Borrowed(ty.display_name()),
    }
}

fn decode_type(ty: &PgTypeInfo, bytes: &[u8]) -> Option<PgDecodedValue> {
    match ty.resolved_kind() {
        Some(PgTypeKind::Domain(base)) => decode_type(base, bytes),
        Some(PgTypeKind::Array(element)) => array::array_value(bytes, &|v| decode_type(element, v)),
        Some(PgTypeKind::Range(element)) => {
            range::range_text(bytes, |v| bound_text(element, v)).map(PgDecodedValue::Text)
        }
        Some(PgTypeKind::Multirange(element)) => {
            range::multirange_text(bytes, |v| bound_text(element, v)).map(PgDecodedValue::Text)
        }
        _ => binary_value(&type_name(ty), bytes).or_else(|| printable_text(bytes)),
    }
}

fn printable_text(bytes: &[u8]) -> Option<PgDecodedValue> {
    let text = std::str::from_utf8(bytes).ok()?;
    text.chars()
        .all(|c| !c.is_control() || c.is_whitespace())
        .then(|| PgDecodedValue::Text(text.to_owned()))
}

fn bound_text(ty: &PgTypeInfo, bytes: &[u8]) -> Option<String> {
    let ty = ty.base_type();
    match ty.display_name() {
        "DATE" => Some(date(bytes)?.text(date_text)),
        "TIMESTAMP" => Some(timestamp(bytes)?.text(|at| timestamp_text(at, ""))),
        "TIMESTAMPTZ" => Some(timestamp(bytes)?.text(|at| timestamp_text(at, "+00"))),
        _ => match decode_type(ty, bytes)? {
            PgDecodedValue::Integer(v) => Some(v.to_string()),
            PgDecodedValue::Numeric(v) | PgDecodedValue::Text(v) => Some(v),
            _ => None,
        },
    }
}

fn binary_value(type_name: &str, bytes: &[u8]) -> Option<PgDecodedValue> {
    use PgDecodedValue as V;
    Some(match type_name {
        "BOOL" => V::Bool(fixed::<1>(bytes)? != [0]),
        "INT2" => V::Integer(i16::from_be_bytes(fixed(bytes)?).into()),
        "INT4" => V::Integer(i32::from_be_bytes(fixed(bytes)?).into()),
        "INT8" => V::Integer(i64::from_be_bytes(fixed(bytes)?)),
        "OID" => V::Integer(u32::from_be_bytes(fixed(bytes)?).into()),
        "FLOAT4" => V::Float(f32::from_be_bytes(fixed(bytes)?).into()),
        "FLOAT8" => V::Float(f64::from_be_bytes(fixed(bytes)?)),
        "NUMERIC" => V::Numeric(numeric::numeric_text(bytes)?),
        "UUID" => V::Text(uuid::Uuid::from_bytes(fixed(bytes)?).to_string()),
        "TEXT" | "VARCHAR" | "CHAR" | "NAME" | "CITEXT" | "UNKNOWN" => {
            V::Text(std::str::from_utf8(bytes).ok()?.to_owned())
        }
        "JSON" => V::Json(serde_json::from_slice(bytes).ok()?),
        "JSONB" => V::Json(serde_json::from_slice(bytes.strip_prefix(&[1])?).ok()?),
        "BYTEA" => V::Bytes(bytes.to_vec()),
        "DATE" => match date(bytes)? {
            Moment::At(v) => V::Date(v),
            Moment::Infinite(v) => V::SpecialMoment(v),
        },
        "TIMESTAMP" => match timestamp(bytes)? {
            Moment::At(v) => V::Timestamp(v),
            Moment::Infinite(v) => V::SpecialMoment(v),
        },
        "TIMESTAMPTZ" => match timestamp(bytes)? {
            Moment::At(v) => V::TimestampWithTimeZone(v.and_utc()),
            Moment::Infinite(v) => V::SpecialMoment(v),
        },
        "TIME" => match i64::from_be_bytes(fixed(bytes)?) {
            MICROS_PER_DAY => V::SpecialMoment("24:00:00"),
            micros => V::Time(time_from_micros(micros).ok()?),
        },
        "TIMETZ" => V::Text(timetz_text(bytes)?),
        "INTERVAL" => V::Text(interval_text(bytes)?),
        "MONEY" => V::Money(PgMoney(i64::from_be_bytes(fixed(bytes)?))),
        "INET" | "CIDR" => V::Text(inet_text(bytes)?),
        _ => return None,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/decoded_tests.rs"]
mod tests;
