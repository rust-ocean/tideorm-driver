//! Fixed-width scalars: `BOOL`, `FLOAT4`, `FLOAT8`, `OID` and `UUID`.
//!
//! In binary format each is its bytes in network order (a `BOOL` is one byte); in text format,
//! what PostgreSQL prints for it.

use std::str::FromStr;

use byteorder::{BigEndian, ByteOrder};
use uuid::Uuid;

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

/// The PostgreSQL [`OID`] type: an object identifier, the key of PostgreSQL's system tables
/// (a table, a type or a function each have one).
///
/// [`OID`]: https://www.postgresql.org/docs/current/datatype-oid.html
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
pub struct Oid(
    /// The identifier, an unsigned 32-bit number on the wire.
    pub u32,
);

/// Each Rust type binds as its PostgreSQL type, its arrays as that type's array, and reads only
/// those.
macro_rules! pg_type {
    ($($rust:ty => $info:ident, $array:ident;)+) => {$(
        impl Type<Postgres> for $rust {
            fn type_info() -> PgTypeInfo {
                PgTypeInfo::$info
            }
        }

        impl PgHasArrayType for $rust {
            fn array_type_info() -> PgTypeInfo {
                PgTypeInfo::$array
            }
        }
    )+};
}

pg_type! {
    bool => BOOL, BOOL_ARRAY;
    f32 => FLOAT4, FLOAT4_ARRAY;
    f64 => FLOAT8, FLOAT8_ARRAY;
    Oid => OID, OID_ARRAY;
    Uuid => UUID, UUID_ARRAY;
}

/// A number in either format: big-endian bytes, read by `read`, or decimal text.
fn decode_number<T>(value: PgValueRef<'_>, read: fn(&[u8]) -> T) -> Result<T, BoxDynError>
where
    T: FromStr,
    BoxDynError: From<T::Err>,
{
    Ok(match value.format() {
        PgValueFormat::Binary => read(value.as_bytes()?),
        PgValueFormat::Text => value.as_str()?.parse()?,
    })
}

impl Encode<'_, Postgres> for bool {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(u8::from(*self));
        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for bool {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.format() {
            PgValueFormat::Binary => Ok(value.as_bytes()?[0] != 0),
            PgValueFormat::Text => match value.as_str()? {
                "t" => Ok(true),
                "f" => Ok(false),
                s => Err(format!("unexpected value {s:?} for boolean").into()),
            },
        }
    }
}

impl Encode<'_, Postgres> for f32 {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(self.to_be_bytes());
        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for f32 {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        decode_number(value, BigEndian::read_f32)
    }
}

impl Encode<'_, Postgres> for f64 {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(self.to_be_bytes());
        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for f64 {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        decode_number(value, BigEndian::read_f64)
    }
}

impl Encode<'_, Postgres> for Oid {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(self.0.to_be_bytes());
        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for Oid {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        decode_number(value, BigEndian::read_u32).map(Oid)
    }
}

impl Encode<'_, Postgres> for Uuid {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend_from_slice(self.as_bytes());
        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for Uuid {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(match value.format() {
            PgValueFormat::Binary => Uuid::from_slice(value.as_bytes()?)?,
            PgValueFormat::Text => value.as_str()?.parse()?,
        })
    }
}
