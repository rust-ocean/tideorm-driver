use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;
use std::mem;

type DefaultTime = ::chrono::NaiveTime;

type DefaultOffset = ::chrono::FixedOffset;

/// Represents a moment of time, in a specified timezone.
///
/// # Warning
///
/// `PgTimeTz` provides `TIMETZ` and is supported only for reading from legacy databases.
/// [PostgreSQL recommends] to use `TIMESTAMPTZ` instead.
///
/// [PostgreSQL recommends]: https://wiki.postgresql.org/wiki/Don't_Do_This#Don.27t_use_timetz
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct PgTimeTz<Time = DefaultTime, Offset = DefaultOffset> {
    pub time: Time,
    pub offset: Offset,
}

impl<Time, Offset> PgHasArrayType for PgTimeTz<Time, Offset> {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::TIMETZ_ARRAY
    }
}

mod chrono {
    use super::*;
    use ::chrono::{FixedOffset, NaiveTime};

    impl Type<Postgres> for PgTimeTz<NaiveTime, FixedOffset> {
        fn type_info() -> PgTypeInfo {
            PgTypeInfo::TIMETZ
        }
    }

    impl Encode<'_, Postgres> for PgTimeTz<NaiveTime, FixedOffset> {
        fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
            let _: IsNull = <NaiveTime as Encode<'_, Postgres>>::encode(self.time, buf)?;
            let _: IsNull =
                <i32 as Encode<'_, Postgres>>::encode(self.offset.utc_minus_local(), buf)?;

            Ok(IsNull::No)
        }

        fn size_hint(&self) -> usize {
            mem::size_of::<i64>() + mem::size_of::<i32>()
        }
    }

    impl<'r> Decode<'r, Postgres> for PgTimeTz<NaiveTime, FixedOffset> {
        fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
            match value.format() {
                PgValueFormat::Binary => {
                    let mut buf = Cursor::new(value.as_bytes()?);

                    // TIME is encoded as the microseconds since midnight
                    let us = buf.read_i64::<BigEndian>()?;
                    let time = crate::types::chrono::time_from_micros(us)?;

                    // OFFSET is encoded as seconds from UTC
                    let offset_seconds = buf.read_i32::<BigEndian>()?;

                    let offset = FixedOffset::west_opt(offset_seconds).ok_or_else(|| {
                        format!(
                            "server returned out-of-range offset for `TIMETZ`: {offset_seconds} seconds"
                        )
                    })?;

                    Ok(PgTimeTz { time, offset })
                }

                PgValueFormat::Text => try_parse_timetz(value.as_str()?),
            }
        }
    }

    fn try_parse_timetz(s: &str) -> Result<PgTimeTz<NaiveTime, FixedOffset>, BoxDynError> {
        // the timestamp parser reads the offset, its seconds too, on a date of no meaning
        let (moment, offset) = crate::types::chrono::parse_text(&format!("2001-07-08 {s}"))?;
        let offset = FixedOffset::east_opt(offset.unwrap_or(0))
            .ok_or_else(|| format!("server returned out-of-range offset for `TIMETZ`: {s}"))?;
        Ok(PgTimeTz {
            time: moment.time(),
            offset,
        })
    }
}
