use byteorder::{BigEndian, ByteOrder};
use std::num::{NonZeroI16, NonZeroI32, NonZeroI64};

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Oid;
use crate::types::Type;
use crate::value::ValueRef;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

/// Whether `ty` holds an integer: any width, or an `OID`, PostgreSQL's one
/// unsigned type. Every Rust integer but `i8` reads a value of any of them
/// that fits in it.
pub(super) fn integer_compatible(ty: &PgTypeInfo) -> bool {
    [
        PgTypeInfo::INT2,
        PgTypeInfo::INT4,
        PgTypeInfo::INT8,
        PgTypeInfo::OID,
    ]
    .contains(ty)
}

/// An array of one of [`integer_compatible`]'s types.
pub(super) fn integer_array_compatible(ty: &PgTypeInfo) -> bool {
    [
        PgTypeInfo::INT2_ARRAY,
        PgTypeInfo::INT4_ARRAY,
        PgTypeInfo::INT8_ARRAY,
        PgTypeInfo::OID_ARRAY,
    ]
    .contains(ty)
}

/// The integer in `value`, an `OID` read as unsigned.
pub(super) fn integer_decode(value: PgValueRef<'_>) -> Result<i64, BoxDynError> {
    if *value.type_info() == PgTypeInfo::OID {
        return Ok(Oid::decode(value)?.0.into());
    }
    int_decode(value)
}

fn int_decode(value: PgValueRef<'_>) -> Result<i64, BoxDynError> {
    Ok(match value.format() {
        PgValueFormat::Text => value.as_str()?.parse()?,
        PgValueFormat::Binary => {
            let buf = value.as_bytes()?;

            // Return error if buf is empty or is more than 8 bytes
            match buf.len() {
                0 => {
                    return Err("Value Buffer found empty while decoding to integer type".into());
                }
                buf_len @ 9.. => {
                    return Err(format!(
                        "Value Buffer exceeds 8 bytes while decoding to integer type. Buffer size = {} bytes ", buf_len
                    )
                    .into());
                }
                _ => {}
            }

            BigEndian::read_int(buf, buf.len())
        }
    })
}

impl Type<Postgres> for i8 {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::CHAR
    }
}

impl PgHasArrayType for i8 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::CHAR_ARRAY
    }
}

impl Encode<'_, Postgres> for i8 {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_be_bytes());

        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for i8 {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        // note: decoding here is for the `"char"` type as Postgres does not have a native 1-byte integer type.
        // https://github.com/postgres/postgres/blob/master/src/backend/utils/adt/char.c#L58-L60
        match value.format() {
            PgValueFormat::Binary => int_decode(value)?.try_into().map_err(Into::into),
            PgValueFormat::Text => {
                let text = value.as_str()?;

                // A value of 0 is represented with the empty string.
                if text.is_empty() {
                    return Ok(0);
                }

                if text.starts_with('\\') {
                    // For values between 0x80 and 0xFF, it's encoded in octal.
                    return Ok(i8::from_str_radix(text.trim_start_matches('\\'), 8)?);
                }

                // Wrapping is the whole idea.
                #[allow(clippy::cast_possible_wrap)]
                Ok(text.as_bytes()[0] as i8)
            }
        }
    }
}

impl Type<Postgres> for i16 {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INT2
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl PgHasArrayType for i16 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INT2_ARRAY
    }

    fn array_compatible(ty: &PgTypeInfo) -> bool {
        integer_array_compatible(ty)
    }
}

impl Encode<'_, Postgres> for i16 {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_be_bytes());

        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for i16 {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        integer_decode(value)?.try_into().map_err(Into::into)
    }
}

impl Type<Postgres> for i32 {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INT4
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl PgHasArrayType for i32 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INT4_ARRAY
    }

    fn array_compatible(ty: &PgTypeInfo) -> bool {
        integer_array_compatible(ty)
    }
}

impl Encode<'_, Postgres> for i32 {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_be_bytes());

        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for i32 {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        integer_decode(value)?.try_into().map_err(Into::into)
    }
}

impl Type<Postgres> for i64 {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INT8
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl PgHasArrayType for i64 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INT8_ARRAY
    }

    fn array_compatible(ty: &PgTypeInfo) -> bool {
        integer_array_compatible(ty)
    }
}

impl Encode<'_, Postgres> for i64 {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_be_bytes());

        Ok(IsNull::No)
    }
}

impl Decode<'_, Postgres> for i64 {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        integer_decode(value)
    }
}

impl PgHasArrayType for NonZeroI16 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INT2_ARRAY
    }
}

impl PgHasArrayType for NonZeroI32 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INT4_ARRAY
    }
}

impl PgHasArrayType for NonZeroI64 {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INT8_ARRAY
    }
}

#[cfg(test)]
#[path = "../../tests/unit/types_int_tests.rs"]
mod tests;
