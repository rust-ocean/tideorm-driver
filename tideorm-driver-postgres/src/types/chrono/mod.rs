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

/// The error for a binary `DATE` or timestamp past chrono's range.
fn out_of_range(type_name: &str, value: i64) -> BoxDynError {
    format!("{type_name} value {value} is outside chrono's range").into()
}

#[cfg(test)]
#[path = "../../../tests/unit/types_chrono_tests.rs"]
mod tests;
