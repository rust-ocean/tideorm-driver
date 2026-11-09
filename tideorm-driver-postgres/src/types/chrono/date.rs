use std::mem;

use chrono::{NaiveDate, TimeDelta};

use super::{out_of_range, postgres_epoch, representable};
use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

impl Type<Postgres> for NaiveDate {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::DATE
    }
}

impl PgHasArrayType for NaiveDate {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::DATE_ARRAY
    }
}

impl Encode<'_, Postgres> for NaiveDate {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        // DATE is encoded as the days since epoch
        let days: i32 = (*self - postgres_epoch().date())
            .num_days()
            .try_into()
            .map_err(|_| {
                format!("value {self:?} would overflow binary encoding for Postgres DATE")
            })?;

        Encode::<Postgres>::encode(days, buf)
    }

    fn size_hint(&self) -> usize {
        mem::size_of::<i32>()
    }
}

impl<'r> Decode<'r, Postgres> for NaiveDate {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = representable(value)?;
        Ok(match value.format() {
            PgValueFormat::Binary => {
                // DATE is encoded as the days since epoch
                let days: i32 = Decode::<Postgres>::decode(value)?;

                TimeDelta::try_days(days.into())
                    .and_then(|days| postgres_epoch().date().checked_add_signed(days))
                    .ok_or_else(|| out_of_range("DATE", days.into()))?
            }

            PgValueFormat::Text => NaiveDate::parse_from_str(value.as_str()?, "%Y-%m-%d")?,
        })
    }
}
