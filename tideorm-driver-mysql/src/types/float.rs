use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::protocol::text::ColumnType;
use crate::types::Type;
use crate::{MySql, MySqlTypeInfo, MySqlValueFormat, MySqlValueRef};

/// A binary value of neither float width: most likely a `DECIMAL`, whose exact
/// digits a float would not keep, so floats do not read it.
fn not_a_float(len: usize, sql_type: &str, rust_type: &str) -> BoxDynError {
    format!(
        "expected a {sql_type} as 4 or 8 bytes, got {len} bytes; \
         note that decoding DECIMAL as `{rust_type}` is not supported \
         due to differing semantics"
    )
    .into()
}

macro_rules! impl_float {
    ($($rust:ty: $column:ident, $sql:literal;)*) => {$(
        impl Type<MySql> for $rust {
            fn type_info() -> MySqlTypeInfo {
                MySqlTypeInfo::binary(ColumnType::$column)
            }

            fn compatible(ty: &MySqlTypeInfo) -> bool {
                // not `DECIMAL`, for the reason above
                matches!(ty.r#type, ColumnType::Float | ColumnType::Double)
            }
        }

        impl Encode<'_, MySql> for $rust {
            fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
                buf.extend_from_slice(&self.to_le_bytes());

                Ok(IsNull::No)
            }
        }

        impl Decode<'_, MySql> for $rust {
            // The server may send a `DOUBLE` for a `FLOAT` column and the other
            // way round, so both widths are read: widened exactly, or rounded
            // as MySQL rounds a `DOUBLE` stored in a `FLOAT`.
            #[allow(clippy::cast_possible_truncation)]
            fn decode(value: MySqlValueRef<'_>) -> Result<Self, BoxDynError> {
                if let MySqlValueFormat::Text = value.format() {
                    return Ok(value.as_str()?.parse()?);
                }

                match *value.as_bytes()? {
                    [a, b, c, d] => Ok(f32::from_le_bytes([a, b, c, d]) as $rust),
                    [a, b, c, d, e, f, g, h] => {
                        Ok(f64::from_le_bytes([a, b, c, d, e, f, g, h]) as $rust)
                    }
                    ref bytes => Err(not_a_float(bytes.len(), $sql, stringify!($rust))),
                }
            }
        }
    )*};
}

impl_float! {
    f32: Float, "FLOAT";
    f64: Double, "DOUBLE";
}

#[cfg(test)]
#[path = "../../tests/unit/types_float_tests.rs"]
mod tests;
