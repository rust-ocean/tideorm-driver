use super::int::{integer_array_compatible, integer_compatible, integer_decode};
use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueRef, Postgres};

// PostgreSQL has no unsigned integers. Each unsigned type is sent as the
// narrowest signed type that holds every value (`u64` as `INT8`, refused past
// `i64::MAX`), and read, as the signed types are, from any integer column or
// `OID` whose value fits.

macro_rules! unsigned {
    ($($unsigned:ty => $signed:ty, $info:ident, $array:ident;)+) => {$(
        impl Type<Postgres> for $unsigned {
            fn type_info() -> PgTypeInfo {
                PgTypeInfo::$info
            }

            fn compatible(ty: &PgTypeInfo) -> bool {
                integer_compatible(ty)
            }
        }

        impl PgHasArrayType for $unsigned {
            fn array_type_info() -> PgTypeInfo {
                PgTypeInfo::$array
            }

            fn array_compatible(ty: &PgTypeInfo) -> bool {
                integer_array_compatible(ty)
            }
        }

        impl Encode<'_, Postgres> for $unsigned {
            fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
                <$signed>::try_from(*self)?.encode_by_ref(buf)
            }
        }

        impl Decode<'_, Postgres> for $unsigned {
            fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
                Ok(integer_decode(value)?.try_into()?)
            }
        }
    )+};
}

unsigned! {
    u16 => i32, INT4, INT4_ARRAY;
    u32 => i64, INT8, INT8_ARRAY;
    u64 => i64, INT8, INT8_ARRAY;
}
