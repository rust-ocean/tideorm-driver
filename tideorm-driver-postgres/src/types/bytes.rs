//! `BYTEA`: byte strings. Binary format carries the bytes as they are; text format prints them
//! as `\x` followed by two hex digits per byte.

use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

// A byte string is the "array" of `u8`: through this impl, the sequence types' `Type` impls
// (`[u8]`, `Vec<u8>`, `[u8; N]`) name `BYTEA`.
impl PgHasArrayType for u8 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::BYTEA
    }
}

array_type!(BYTEA_ARRAY: [] &'_ [u8], [] Box<[u8]>, [] Vec<u8>, [const N: usize] [u8; N]);

impl Encode<'_, Postgres> for &'_ [u8] {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend_from_slice(self);
        Ok(IsNull::No)
    }
}

impl Encode<'_, Postgres> for Vec<u8> {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        <&[u8] as Encode<Postgres>>::encode(self, buf)
    }
}

impl<const N: usize> Encode<'_, Postgres> for [u8; N] {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        <&[u8] as Encode<Postgres>>::encode(self.as_slice(), buf)
    }
}

forward_encode_impl!(Arc<[u8]>, &[u8], Postgres);
forward_encode_impl!(Rc<[u8]>, &[u8], Postgres);
forward_encode_impl!(Box<[u8]>, &[u8], Postgres);
forward_encode_impl!(Cow<'_, [u8]>, &[u8], Postgres);

impl<'r> Decode<'r, Postgres> for &'r [u8] {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        match value.format() {
            PgValueFormat::Binary => value.as_bytes(),
            // The text form is hex, so there are no raw bytes to borrow.
            PgValueFormat::Text => Err("unsupported decode to `&[u8]` of BYTEA in a simple query; use a prepared query or decode to `Vec<u8>`".into()),
        }
    }
}

impl Decode<'_, Postgres> for Vec<u8> {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(match value.format() {
            PgValueFormat::Binary => value.as_bytes()?.to_vec(),
            PgValueFormat::Text => hex::decode(hex_digits(value)?)?,
        })
    }
}

impl<const N: usize> Decode<'_, Postgres> for [u8; N] {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.format() {
            PgValueFormat::Binary => Ok(value.as_bytes()?.try_into()?),
            PgValueFormat::Text => {
                let mut bytes = [0; N];
                hex::decode_to_slice(hex_digits(value)?, &mut bytes)?;
                Ok(bytes)
            }
        }
    }
}

/// The hex digits of a `BYTEA` in text format, after its `\x` prefix.
fn hex_digits(value: PgValueRef<'_>) -> Result<&[u8], BoxDynError> {
    value
        .as_bytes()?
        .strip_prefix(b"\\x")
        .ok_or_else(|| "text does not start with \\x".into())
}
