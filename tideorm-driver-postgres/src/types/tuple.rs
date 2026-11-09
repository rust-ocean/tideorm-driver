//! Anonymous records as tuples, and `()` for `VOID`.
//!
//! PostgreSQL returns an anonymous record for a row constructor (`SELECT (1, 'a')`) or a
//! function declared to return `record`, but takes none as a parameter, so tuples only decode.

use crate::decode::Decode;
use crate::error::BoxDynError;
use crate::types::{PgRecordDecoder, Type};
use crate::{PgHasArrayType, PgTypeInfo, PgValueRef, Postgres};

/// Implements the record impls for the tuple of all the listed type parameters, then for each
/// shorter tuple, dropping one parameter at a time from the front.
macro_rules! tuples {
    (@record $($field:ident)+) => {
        impl<$($field),+> Type<Postgres> for ($($field,)+) {
            fn type_info() -> PgTypeInfo {
                PgTypeInfo::RECORD
            }
        }

        impl<$($field),+> PgHasArrayType for ($($field,)+) {
            fn array_type_info() -> PgTypeInfo {
                PgTypeInfo::RECORD_ARRAY
            }
        }

        impl<'r, $($field),+> Decode<'r, Postgres> for ($($field,)+)
        where
            $($field: 'r + Type<Postgres> + for<'a> Decode<'a, Postgres>,)+
        {
            fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
                let mut record = PgRecordDecoder::new(value)?;
                // A tuple expression is evaluated left to right: fields are read in order.
                Ok(($(record.try_decode::<$field>()?,)+))
            }
        }
    };
    () => {};
    ($first:ident $($rest:ident)*) => {
        tuples!(@record $first $($rest)*);
        tuples!($($rest)*);
    };
}

tuples!(T1 T2 T3 T4 T5 T6 T7 T8 T9);

// `()` reads what a function returning `void` gives, and the empty record.
impl Type<Postgres> for () {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::VOID
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        *ty == PgTypeInfo::VOID || *ty == PgTypeInfo::RECORD
    }
}

impl Decode<'_, Postgres> for () {
    fn decode(_value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(())
    }
}
