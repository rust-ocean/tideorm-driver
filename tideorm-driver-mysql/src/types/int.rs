use byteorder::{ByteOrder, LittleEndian};

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::protocol::text::{ColumnFlags, ColumnType};
use crate::types::Type;
use crate::{MySql, MySqlTypeInfo, MySqlValueFormat, MySqlValueRef};

/// Whether `ty` holds an integer, signed or not. Every Rust integer type
/// reads a value of any of them that fits in it.
pub(super) fn integer_compatible(ty: &MySqlTypeInfo) -> bool {
    matches!(
        ty.r#type,
        ColumnType::Tiny
            | ColumnType::Short
            | ColumnType::Long
            | ColumnType::Int24
            | ColumnType::LongLong
            | ColumnType::Year
            | ColumnType::Bit
    )
}

impl Type<MySql> for i8 {
    fn type_info() -> MySqlTypeInfo {
        MySqlTypeInfo::binary(ColumnType::Tiny)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl Type<MySql> for i16 {
    fn type_info() -> MySqlTypeInfo {
        MySqlTypeInfo::binary(ColumnType::Short)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl Type<MySql> for i32 {
    fn type_info() -> MySqlTypeInfo {
        MySqlTypeInfo::binary(ColumnType::Long)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl Type<MySql> for i64 {
    fn type_info() -> MySqlTypeInfo {
        MySqlTypeInfo::binary(ColumnType::LongLong)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl Encode<'_, MySql> for i8 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_le_bytes());

        Ok(IsNull::No)
    }
}

impl Encode<'_, MySql> for i16 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_le_bytes());

        Ok(IsNull::No)
    }
}

impl Encode<'_, MySql> for i32 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_le_bytes());

        Ok(IsNull::No)
    }
}

impl Encode<'_, MySql> for i64 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_le_bytes());

        Ok(IsNull::No)
    }
}

/// The integer in `value`, its bytes read by the column's signedness, so an
/// `UNSIGNED` value past the signed range keeps its value. A `BIT` value is
/// big-endian bytes in either format.
pub(super) fn integer_decode(value: MySqlValueRef<'_>) -> Result<i128, BoxDynError> {
    if value.type_info.r#type == ColumnType::Bit {
        let bits = value
            .as_bytes()?
            .iter()
            .fold(0u64, |bits, byte| (bits << 8) | u64::from(*byte));
        return Ok(bits.into());
    }

    Ok(match value.format() {
        MySqlValueFormat::Text => value.as_str()?.parse()?,
        MySqlValueFormat::Binary => {
            let buf = value.as_bytes()?;

            // Check conditions that could cause `read_int()` to panic.
            if buf.is_empty() {
                return Err("empty buffer".into());
            }

            if buf.len() > 8 {
                return Err(format!(
                    "expected no more than 8 bytes for integer value, got {}",
                    buf.len()
                )
                .into());
            }

            if value.type_info.flags.contains(ColumnFlags::UNSIGNED) {
                LittleEndian::read_uint(buf, buf.len()).into()
            } else {
                LittleEndian::read_int(buf, buf.len()).into()
            }
        }
    })
}

macro_rules! integer_decoders {
    ($($integer:ty),+) => {$(
        impl Decode<'_, MySql> for $integer {
            fn decode(value: MySqlValueRef<'_>) -> Result<Self, BoxDynError> {
                integer_decode(value)?.try_into().map_err(Into::into)
            }
        }
    )+};
}

integer_decoders!(i8, i16, i32, i64, u8, u16, u32, u64);

#[cfg(test)]
#[path = "../../tests/unit/types_int_tests.rs"]
mod tests;
