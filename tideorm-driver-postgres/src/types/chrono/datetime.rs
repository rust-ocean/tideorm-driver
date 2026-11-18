use super::{out_of_range, parse_text, postgres_epoch, representable};
use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use chrono::{DateTime, Duration, FixedOffset, Local, NaiveDateTime, Offset, TimeZone, Utc};
use std::mem;

impl Type<Postgres> for NaiveDateTime {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::TIMESTAMP
    }
}

impl<Tz: TimeZone> Type<Postgres> for DateTime<Tz> {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::TIMESTAMPTZ
    }
}

impl PgHasArrayType for NaiveDateTime {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::TIMESTAMP_ARRAY
    }
}

impl<Tz: TimeZone> PgHasArrayType for DateTime<Tz> {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::TIMESTAMPTZ_ARRAY
    }
}

impl Encode<'_, Postgres> for NaiveDateTime {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        // TIMESTAMP is encoded as the microseconds since the epoch
        let micros = (*self - postgres_epoch())
            .num_microseconds()
            .ok_or_else(|| format!("NaiveDateTime out of range for Postgres: {self:?}"))?;

        Encode::<Postgres>::encode(micros, buf)
    }

    fn size_hint(&self) -> usize {
        mem::size_of::<i64>()
    }
}

impl<'r> Decode<'r, Postgres> for NaiveDateTime {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = representable(value)?;
        Ok(match value.format() {
            PgValueFormat::Binary => {
                // TIMESTAMP is encoded as the microseconds since the epoch
                let us = Decode::<Postgres>::decode(value)?;
                postgres_epoch()
                    .checked_add_signed(Duration::microseconds(us))
                    .ok_or_else(|| out_of_range("timestamp", us))?
            }

            // a `TIMESTAMPTZ` prints in the session's time zone; read as UTC, as in binary
            PgValueFormat::Text => match parse_text(value.as_str()?)? {
                (naive, Some(offset)) => naive - Duration::seconds(offset.into()),
                (naive, None) => naive,
            },
        })
    }
}

impl<Tz: TimeZone> Encode<'_, Postgres> for DateTime<Tz> {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        Encode::<Postgres>::encode(self.naive_utc(), buf)
    }

    fn size_hint(&self) -> usize {
        mem::size_of::<i64>()
    }
}

impl<'r> Decode<'r, Postgres> for DateTime<Local> {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let fixed = <DateTime<FixedOffset> as Decode<Postgres>>::decode(value)?;
        Ok(Local.from_utc_datetime(&fixed.naive_utc()))
    }
}

impl<'r> Decode<'r, Postgres> for DateTime<Utc> {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let fixed = <DateTime<FixedOffset> as Decode<Postgres>>::decode(value)?;
        Ok(Utc.from_utc_datetime(&fixed.naive_utc()))
    }
}

impl<'r> Decode<'r, Postgres> for DateTime<FixedOffset> {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = representable(value)?;
        Ok(match value.format() {
            PgValueFormat::Binary => {
                let naive = <NaiveDateTime as Decode<Postgres>>::decode(value)?;
                Utc.fix().from_utc_datetime(&naive)
            }

            // the offset of the session's time zone it printed in
            PgValueFormat::Text => {
                let (naive, offset) = parse_text(value.as_str()?)?;
                let offset = FixedOffset::east_opt(offset.unwrap_or(0))
                    .ok_or_else(|| format!("invalid offset {offset:?}"))?;
                offset
                    .from_local_datetime(&naive)
                    .single()
                    .ok_or("timestamp out of range")?
            }
        })
    }
}
