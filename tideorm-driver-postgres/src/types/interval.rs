use std::mem;

use byteorder::{NetworkEndian, ReadBytesExt};

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

// `PgInterval` is available for direct access to the INTERVAL type

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash, Default)]
pub struct PgInterval {
    pub months: i32,
    pub days: i32,
    pub microseconds: i64,
}

impl Type<Postgres> for PgInterval {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL
    }
}

impl PgHasArrayType for PgInterval {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL_ARRAY
    }
}

impl PgInterval {
    /// An `INTERVAL` in its binary form: microseconds, days, then months.
    pub(crate) fn from_binary(mut bytes: &[u8]) -> Result<Self, BoxDynError> {
        let microseconds = bytes.read_i64::<NetworkEndian>()?;
        let days = bytes.read_i32::<NetworkEndian>()?;
        let months = bytes.read_i32::<NetworkEndian>()?;
        if !bytes.is_empty() {
            return Err(format!("{} bytes past the end of an INTERVAL", bytes.len()).into());
        }
        Ok(PgInterval {
            months,
            days,
            microseconds,
        })
    }
}

impl<'de> Decode<'de, Postgres> for PgInterval {
    fn decode(value: PgValueRef<'de>) -> Result<Self, BoxDynError> {
        match value.format() {
            PgValueFormat::Binary => Self::from_binary(value.as_bytes()?),

            PgValueFormat::Text => {
                Err("not implemented: decode `INTERVAL` in text mode (unprepared queries)".into())
            }
        }
    }
}

impl Encode<'_, Postgres> for PgInterval {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.microseconds.to_be_bytes());
        buf.extend(&self.days.to_be_bytes());
        buf.extend(&self.months.to_be_bytes());

        Ok(IsNull::No)
    }

    fn size_hint(&self) -> usize {
        2 * mem::size_of::<i64>()
    }
}

// Encode and Type are implemented for standard and chrono durations for convenience.

impl Type<Postgres> for std::time::Duration {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL
    }
}

impl PgHasArrayType for std::time::Duration {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL_ARRAY
    }
}

impl Encode<'_, Postgres> for std::time::Duration {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        PgInterval::try_from(*self)?.encode_by_ref(buf)
    }

    fn size_hint(&self) -> usize {
        2 * mem::size_of::<i64>()
    }
}

impl TryFrom<std::time::Duration> for PgInterval {
    type Error = BoxDynError;

    /// Convert a `std::time::Duration` to a `PgInterval`
    ///
    /// This returns an error if there is a loss of precision using nanoseconds or if there is a
    /// microsecond overflow.
    fn try_from(value: std::time::Duration) -> Result<Self, BoxDynError> {
        if !value.as_nanos().is_multiple_of(1000) {
            return Err("PostgreSQL `INTERVAL` does not support nanoseconds precision".into());
        }

        Ok(Self {
            months: 0,
            days: 0,
            microseconds: value.as_micros().try_into()?,
        })
    }
}

impl Type<Postgres> for chrono::Duration {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL
    }
}

impl PgHasArrayType for chrono::Duration {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL_ARRAY
    }
}

impl Encode<'_, Postgres> for chrono::Duration {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        let pg_interval = PgInterval::try_from(*self)?;
        pg_interval.encode_by_ref(buf)
    }

    fn size_hint(&self) -> usize {
        2 * mem::size_of::<i64>()
    }
}

impl TryFrom<chrono::Duration> for PgInterval {
    type Error = BoxDynError;

    /// Convert a `chrono::Duration` to a `PgInterval`.
    ///
    /// This returns an error if there is a loss of precision using nanoseconds or if there is a
    /// nanosecond overflow.
    fn try_from(value: chrono::Duration) -> Result<Self, BoxDynError> {
        value
            .num_nanoseconds()
            .map_or::<Result<_, Self::Error>, _>(
                Err("Overflow has occurred for PostgreSQL `INTERVAL`".into()),
                |nanoseconds| {
                    if nanoseconds % 1000 != 0 {
                        return Err(
                            "PostgreSQL `INTERVAL` does not support nanoseconds precision".into(),
                        );
                    }
                    Ok(())
                },
            )?;

        value.num_microseconds().map_or(
            Err("Overflow has occurred for PostgreSQL `INTERVAL`".into()),
            |microseconds| {
                Ok(Self {
                    months: 0,
                    days: 0,
                    microseconds,
                })
            },
        )
    }
}

#[cfg(test)]
#[path = "../../tests/unit/types_interval_tests.rs"]
mod tests;
