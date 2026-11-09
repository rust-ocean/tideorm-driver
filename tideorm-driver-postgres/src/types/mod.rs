//! Conversions between Rust and **Postgres** types.
//!
//! # Types
//!
//! | Rust type                             | Postgres type(s)                                     |
//! |---------------------------------------|------------------------------------------------------|
//! | `bool`                                | BOOL                                                 |
//! | `i8`                                  | "CHAR"                                               |
//! | `i16`                                 | SMALLINT, SMALLSERIAL, INT2                          |
//! | `i32`                                 | INT, SERIAL, INT4                                    |
//! | `i64`                                 | BIGINT, BIGSERIAL, INT8                              |
//! | `u16`, `u32`, `u64`                   | INT4, INT8, INT8                                     |
//!
//! Every integer type but `i8` is read from any integer column or `OID`
//! whose value fits it, and its `Vec` from an array of one.
//! | `f32`                                 | REAL, FLOAT4                                         |
//! | `f64`                                 | DOUBLE PRECISION, FLOAT8                             |
//! | `&str`, [`String`]                    | VARCHAR, CHAR(N), TEXT, NAME, CITEXT                 |
//! | `&[u8]`, `Vec<u8>`                    | BYTEA                                                |
//! | `()`                                  | VOID                                                 |
//! | [`PgInterval`]                        | INTERVAL                                             |
//! | [`PgRange<T>`](PgRange)               | INT8RANGE, INT4RANGE, TSRANGE, TSTZRANGE, DATERANGE, NUMRANGE |
//! | [`PgMoney`]                           | MONEY                                                |
//! | [`PgLTree`]                           | LTREE                                                |
//! | [`PgLQuery`]                          | LQUERY                                               |
//! | [`PgCiText`]                          | CITEXT<sup>1</sup>                                   |
//! | [`PgCube`]                            | CUBE                                                 |
//! | [`PgPoint`]                           | POINT                                                |
//! | [`PgLine`]                            | LINE                                                 |
//! | [`PgLSeg`]                            | LSEG                                                 |
//! | [`PgBox`]                             | BOX                                                  |
//! | [`PgPath`]                            | PATH                                                 |
//! | [`PgPolygon`]                         | POLYGON                                              |
//! | [`PgCircle`]                          | CIRCLE                                               |
//! | [`PgHstore`]                          | HSTORE                                               |
//!
//! <sup>1</sup> The driver generally considers `CITEXT` to be compatible with `String`,
//! `&str`, etc., but this wrapper type is available for edge cases, such as `CITEXT[]`
//! which Postgres does not consider to be compatible with `TEXT[]`.
//!
//! ### [`rust_decimal`](https://crates.io/crates/rust_decimal)
//! | Rust type                             | Postgres type(s)                                        |
//! |---------------------------------------|------------------------------------------------------|
//! | `rust_decimal::Decimal`               | NUMERIC                                              |
//!
#![doc=include_str!("rust_decimal-range.md")]
//!
//! ### [`chrono`](https://crates.io/crates/chrono)
//!
//! | Rust type                             | Postgres type(s)                                     |
//! |---------------------------------------|------------------------------------------------------|
//! | `chrono::DateTime<Utc>`               | TIMESTAMPTZ                                          |
//! | `chrono::DateTime<Local>`             | TIMESTAMPTZ                                          |
//! | `chrono::NaiveDateTime`               | TIMESTAMP                                            |
//! | `chrono::NaiveDate`                   | DATE                                                 |
//! | `chrono::NaiveTime`                   | TIME                                                 |
//! | [`PgTimeTz`]                          | TIMETZ                                               |
//!
//! ### [`uuid`](https://crates.io/crates/uuid)
//!
//! | Rust type                             | Postgres type(s)                                     |
//! |---------------------------------------|------------------------------------------------------|
//! | `uuid::Uuid`                          | UUID                                                 |
//!
//! ### [`json`](https://crates.io/crates/serde_json)
//!
//! | Rust type                             | Postgres type(s)                                     |
//! |---------------------------------------|------------------------------------------------------|
//! | [`Json<T>`](driver_core::types::Json)                           | JSON, JSONB                                          |
//! | `serde_json::Value`                   | JSON, JSONB                                          |
//! | `&serde_json::value::RawValue`        | JSON, JSONB                                          |
//!
//! `Value` and `RawValue` from `serde_json` can be used for unstructured JSON data with
//! Postgres.
//!
//! [`Json<T>`](driver_core::types::Json) can be used for structured JSON data with Postgres.
//!
//! # [Composite types](https://www.postgresql.org/docs/current/rowtypes.html)
//!
//! Anonymous composite types are represented as tuples. Note that anonymous composites may only
//! be returned and not sent to Postgres (this is a limitation of Postgres).
//!
//! # Reading any value
//!
//! [`PgValueRef::decode_lossless`](crate::PgValueRef::decode_lossless) reads a value of any type
//! as a [`PgDecodedValue`] without narrowing it: exact numerics, `infinity` dates and timestamps,
//! `TIME '24:00:00'`, ranges and multiranges as PostgreSQL prints them, and arrays with their
//! NULLs.
//!
//! # Arrays
//!
//! One-dimensional arrays are supported as `Vec<T>` or `&[T]` where `T` implements `Type`.
//!

use crate::type_info::PgTypeKind;
use crate::{PgTypeInfo, Postgres};

pub(crate) use driver_core::types::Type;

/// Implements [`PgHasArrayType`] for each `[generics] type`: its arrays bind as
/// `PgTypeInfo::$array` and read only that type, or, with `of $element`, any array whose element
/// type `$element` reads.
macro_rules! array_type {
    ($array:ident: $([$($generics:tt)*] $rust:ty),+ $(,)?) => {$(
        impl<$($generics)*> $crate::PgHasArrayType for $rust {
            fn array_type_info() -> $crate::PgTypeInfo {
                $crate::PgTypeInfo::$array
            }
        }
    )+};
    ($array:ident of $element:ty: $([$($generics:tt)*] $rust:ty),+ $(,)?) => {$(
        impl<$($generics)*> $crate::PgHasArrayType for $rust {
            fn array_type_info() -> $crate::PgTypeInfo {
                $crate::PgTypeInfo::$array
            }

            fn array_compatible(ty: &$crate::PgTypeInfo) -> bool {
                $crate::types::array_compatible::<$element>(ty)
            }
        }
    )+};
}

mod array;
mod bytes;
mod citext;
mod hstore;
mod int;
mod interval;
mod json;
mod lquery;
mod ltree;
mod money;
mod range;
mod record;
mod scalar;
mod str;
mod tuple;
mod uint;

mod time_tz;

mod cube;

mod decoded;

mod geometry;

mod numeric;

mod rust_decimal;

mod chrono;

pub use array::PgHasArrayType;
pub use chrono::PgUnrepresentableMoment;
pub use citext::PgCiText;
pub use cube::PgCube;
pub use decoded::{PgDecodedArray, PgDecodedValue};
pub use geometry::circle::PgCircle;
pub use geometry::line::PgLine;
pub use geometry::line_segment::PgLSeg;
pub use geometry::path::PgPath;
pub use geometry::point::PgPoint;
pub use geometry::polygon::PgPolygon;
pub use geometry::r#box::PgBox;
pub use hstore::PgHstore;
pub use interval::PgInterval;
pub use lquery::PgLQuery;
pub use lquery::PgLQueryLevel;
pub use lquery::PgLQueryVariant;
pub use lquery::PgLQueryVariantFlag;
pub use ltree::PgLTree;
pub use ltree::PgLTreeLabel;
pub use ltree::PgLTreeParseError;
pub use money::PgMoney;
pub use range::PgRange;
pub use scalar::Oid;

pub use time_tz::PgTimeTz;

// Internal building blocks for composite type encoding and decoding.
#[doc(hidden)]
pub use record::{PgRecordDecoder, PgRecordEncoder};

// Type::compatible impl appropriate for arrays
fn array_compatible<E: Type<Postgres> + ?Sized>(ty: &PgTypeInfo) -> bool {
    // we require the declared type to be an _array_ with an
    // element type that is acceptable
    if let PgTypeKind::Array(element) = &ty.kind() {
        return E::compatible(element);
    }

    false
}
