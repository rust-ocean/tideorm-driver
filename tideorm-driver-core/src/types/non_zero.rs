//! [`Type`], [`Encode`] and [`Decode`] for [`NonZero`] integers, on every backend that
//! has them for the plain integer: a `NonZero<T>` binds and reads as a `T`, and decoding
//! a zero fails.

use std::num::NonZero;

use crate::database::Database;
use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;

macro_rules! impl_non_zero {
    ($($int:ty),* $(,)?) => {$(
        impl<DB> Type<DB> for NonZero<$int>
        where
            DB: Database,
            $int: Type<DB>,
        {
            fn type_info() -> <DB as Database>::TypeInfo {
                <$int as Type<DB>>::type_info()
            }

            fn compatible(ty: &<DB as Database>::TypeInfo) -> bool {
                <$int as Type<DB>>::compatible(ty)
            }
        }

        impl<'q, DB> Encode<'q, DB> for NonZero<$int>
        where
            DB: Database,
            $int: Encode<'q, DB>,
        {
            fn encode_by_ref(
                &self,
                buf: &mut <DB as Database>::ArgumentBuffer,
            ) -> Result<IsNull, BoxDynError> {
                <$int as Encode<'q, DB>>::encode_by_ref(&self.get(), buf)
            }

            fn encode(self, buf: &mut <DB as Database>::ArgumentBuffer) -> Result<IsNull, BoxDynError>
            where
                Self: Sized,
            {
                <$int as Encode<'q, DB>>::encode(self.get(), buf)
            }

            fn produces(&self) -> Option<<DB as Database>::TypeInfo> {
                <$int as Encode<'q, DB>>::produces(&self.get())
            }
        }

        impl<'r, DB> Decode<'r, DB> for NonZero<$int>
        where
            DB: Database,
            $int: Decode<'r, DB>,
        {
            /// Fails with [`std::num::TryFromIntError`] on a zero.
            fn decode(value: <DB as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
                Ok(Self::try_from(<$int as Decode<'r, DB>>::decode(value)?)?)
            }
        }
    )*};
}

impl_non_zero!(i8, u8, i16, u16, i32, u32, i64, u64);
