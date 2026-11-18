use std::fmt;

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};

use crate::error::BoxDynError;
use crate::PgValueRef;

mod date;
mod datetime;
mod time;

/// Microseconds in a day: `TIME '24:00:00'`, the one `TIME` value past every `NaiveTime`.
pub(crate) const MICROS_PER_DAY: i64 = 86_400_000_000;

/// PostgreSQL's epoch, 2000-01-01 00:00:00, which its dates and timestamps count from.
pub(crate) fn postgres_epoch() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2000, 1, 1)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .expect("2000-01-01 00:00:00 is a valid NaiveDateTime")
}

/// The `NaiveTime` `us` microseconds after midnight, refusing `24:00:00` rather than wrapping
/// it round to midnight.
pub(crate) fn time_from_micros(us: i64) -> Result<NaiveTime, BoxDynError> {
    if !(0..MICROS_PER_DAY).contains(&us) {
        return Err(if us == MICROS_PER_DAY {
            "TIME '24:00:00' has no `NaiveTime` equivalent".into()
        } else {
            format!("TIME of {us} microseconds after midnight is out of range").into()
        });
    }
    // Midnight is the default; the `NaiveTime` decoder's tests check it.
    Ok(NaiveTime::default() + Duration::microseconds(us))
}

/// A PostgreSQL date, time or timestamp no Rust date or time type can hold: a `DATE` or
/// timestamp holding `infinity` or `-infinity`, or `TIME '24:00:00'`.
///
/// The chrono decoders return it as their error, so a caller can tell it from other decode
/// failures; [`PgValueRef::decode_lossless`] reads such a value as its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PgUnrepresentableMoment {
    pub(crate) type_name: &'static str,
    pub(crate) value: &'static str,
}

impl PgUnrepresentableMoment {
    /// The value's type: `DATE`, `TIME`, `TIMESTAMP` or `TIMESTAMPTZ`.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// The value as PostgreSQL prints it: `infinity`, `-infinity` or `24:00:00`.
    pub fn value(&self) -> &'static str {
        self.value
    }
}

impl fmt::Display for PgUnrepresentableMoment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} '{}' has no Rust date or time equivalent",
            self.type_name, self.value
        )
    }
}

impl std::error::Error for PgUnrepresentableMoment {}

/// `value`, unless it is a moment no chrono type can hold.
fn representable(value: PgValueRef<'_>) -> Result<PgValueRef<'_>, PgUnrepresentableMoment> {
    match value.unrepresentable_moment() {
        Some(moment) => Err(moment),
        None => Ok(value),
    }
}

/// A `DATE`, `TIMESTAMP`, `TIMESTAMPTZ` or dated `TIMETZ` as PostgreSQL prints it in the ISO
/// style the connection sets: a year of four digits or more, the time and the offset (`+05`,
/// `-04:56:02`) if any, then ` BC` for a year before 1. The moment as written, and its offset in
/// seconds.
pub(super) fn parse_text(text: &str) -> Result<(NaiveDateTime, Option<i32>), BoxDynError> {
    let (text, bc) = match text.strip_suffix(" BC") {
        Some(text) => (text, true),
        None => (text, false),
    };
    let (date, rest) = text.split_once(' ').unwrap_or((text, ""));
    let mut fields = date.splitn(3, '-').map(str::parse::<u32>);
    let mut field = || fields.next().unwrap_or(Ok(0));
    let (year, month, day) = (i32::try_from(field()?)?, field()?, field()?);
    // 1 BC is the year 0
    let year = if bc { 1 - year } else { year };
    let date = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("invalid date {date:?}"))?;

    let (time, offset) = match rest.find(['+', '-']) {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    let time = match time {
        "" => NaiveTime::MIN,
        time => NaiveTime::parse_from_str(time, "%H:%M:%S%.f")?,
    };
    let offset = match offset.split_at_checked(1) {
        Some((sign, offset)) => {
            let mut seconds = 0;
            for (part, unit) in offset.split(':').zip([3600, 60, 1]) {
                seconds += part.parse::<i32>()? * unit;
            }
            Some(if sign == "-" { -seconds } else { seconds })
        }
        None => None,
    };
    Ok((date.and_time(time), offset))
}

/// The error for a binary `DATE` or timestamp past chrono's range.
fn out_of_range(type_name: &str, value: i64) -> BoxDynError {
    format!("{type_name} value {value} is outside chrono's range").into()
}

#[cfg(test)]
#[path = "../../../tests/unit/types_chrono_tests.rs"]
mod tests;
