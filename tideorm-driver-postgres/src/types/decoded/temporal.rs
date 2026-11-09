use super::wire::fixed;
use crate::types::chrono::postgres_epoch;
use crate::types::PgInterval;
use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
use std::fmt::Write;

/// A date or a timestamp, or one of the infinities PostgreSQL keeps as the
/// extreme values of its form.
pub(super) enum Moment<T> {
    At(T),
    Infinite(&'static str),
}

impl<T> Moment<T> {
    pub(super) fn text(self, finite: impl FnOnce(T) -> String) -> String {
        match self {
            Moment::At(at) => finite(at),
            Moment::Infinite(infinity) => infinity.to_string(),
        }
    }
}

/// A `DATE`: days since the epoch.
pub(super) fn date(bytes: &[u8]) -> Option<Moment<NaiveDate>> {
    match i32::from_be_bytes(fixed(bytes)?) {
        i32::MAX => Some(Moment::Infinite("infinity")),
        i32::MIN => Some(Moment::Infinite("-infinity")),
        days => postgres_epoch()
            .date()
            .checked_add_signed(TimeDelta::try_days(days.into())?)
            .map(Moment::At),
    }
}

/// A `TIMESTAMP` or `TIMESTAMPTZ`: microseconds since the epoch, in UTC.
pub(super) fn timestamp(bytes: &[u8]) -> Option<Moment<NaiveDateTime>> {
    match i64::from_be_bytes(fixed(bytes)?) {
        i64::MAX => Some(Moment::Infinite("infinity")),
        i64::MIN => Some(Moment::Infinite("-infinity")),
        micros => postgres_epoch()
            .checked_add_signed(TimeDelta::microseconds(micros))
            .map(Moment::At),
    }
}

/// Microseconds since midnight as `HH:MM:SS`, then any fraction of a second
/// without its trailing zeros.
pub(super) fn time_text(micros: i64) -> String {
    let mut text = format!(
        "{:02}:{:02}:{:02}",
        micros / 3_600_000_000,
        micros / 60_000_000 % 60,
        micros / 1_000_000 % 60
    );
    push_fraction(&mut text, (micros % 1_000_000).unsigned_abs());
    text
}

fn push_fraction(text: &mut String, micros: u64) {
    if micros != 0 {
        let _ = write!(text, ".{micros:06}");
        text.truncate(text.trim_end_matches('0').len());
    }
}

/// A date as PostgreSQL prints one, with a year of four digits or more, and
/// the ` BC` it ends a date before year 1 with.
fn date_parts(date: NaiveDate) -> (String, &'static str) {
    use chrono::Datelike;
    let (year, era) = match date.year() {
        year if year > 0 => (year, ""),
        year => (1 - year, " BC"),
    };
    let text = format!("{year:04}-{:02}-{:02}", date.month(), date.day());
    (text, era)
}

pub(super) fn date_text(date: NaiveDate) -> String {
    let (date, era) = date_parts(date);
    date + era
}

/// A timestamp as PostgreSQL prints one, with `zone` after the time.
pub(super) fn timestamp_text(at: NaiveDateTime, zone: &str) -> String {
    use chrono::Timelike;
    let (date, era) = date_parts(at.date());
    let time = at.time();
    let micros = i64::from(time.num_seconds_from_midnight()) * 1_000_000
        + i64::from(time.nanosecond() / 1_000);
    format!("{date} {}{zone}{era}", time_text(micros))
}

/// A `TIMETZ`: microseconds since midnight, then the zone in seconds west of
/// UTC, printed as `+HH`, `+HH:MM` or `+HH:MM:SS` east of it.
pub(super) fn timetz_text(bytes: &[u8]) -> Option<String> {
    let (time, zone) = bytes.split_first_chunk::<8>()?;
    let zone = i32::from_be_bytes(fixed(zone)?);
    let mut text = time_text(i64::from_be_bytes(*time));
    let seconds = zone.unsigned_abs();
    text.push(if zone <= 0 { '+' } else { '-' });
    let _ = write!(text, "{:02}", seconds / 3600);
    if seconds % 60 != 0 {
        let _ = write!(text, ":{:02}:{:02}", seconds / 60 % 60, seconds % 60);
    } else if seconds / 60 % 60 != 0 {
        let _ = write!(text, ":{:02}", seconds / 60 % 60);
    }
    Some(text)
}

/// An `INTERVAL`: microseconds, days, then months, each counted apart, as
/// PostgreSQL writes it under `intervalstyle = iso_8601`.
pub(super) fn interval_text(bytes: &[u8]) -> Option<String> {
    let PgInterval {
        months,
        days,
        microseconds: micros,
    } = PgInterval::from_binary(bytes).ok()?;
    // PostgreSQL 17's infinite intervals.
    match (micros, days, months) {
        (i64::MAX, i32::MAX, i32::MAX) => return Some("infinity".to_string()),
        (i64::MIN, i32::MIN, i32::MIN) => return Some("-infinity".to_string()),
        (0, 0, 0) => return Some("PT0S".to_string()),
        _ => {}
    }
    // Each part keeps the sign of what it is taken from, as PostgreSQL's
    // truncating division leaves it.
    let mut text = String::from("P");
    for (value, unit) in [
        (i64::from(months / 12), 'Y'),
        (i64::from(months % 12), 'M'),
        (i64::from(days), 'D'),
    ] {
        if value != 0 {
            let _ = write!(text, "{value}{unit}");
        }
    }
    if micros != 0 {
        text.push('T');
        for (value, unit) in [
            (micros / 3_600_000_000, 'H'),
            (micros / 60_000_000 % 60, 'M'),
        ] {
            if value != 0 {
                let _ = write!(text, "{value}{unit}");
            }
        }
        let (seconds, fraction) = (micros / 1_000_000 % 60, micros % 1_000_000);
        if seconds != 0 || fraction != 0 {
            if seconds < 0 || fraction < 0 {
                text.push('-');
            }
            let _ = write!(text, "{}", seconds.unsigned_abs());
            push_fraction(&mut text, fraction.unsigned_abs());
            text.push('S');
        }
    }
    Some(text)
}

/// An `INET` or `CIDR`: its family, mask length, whether it is a `CIDR`, the
/// address length, then the address. An `INET` names its mask only when it
/// does not cover the whole address.
pub(super) fn inet_text(bytes: &[u8]) -> Option<String> {
    let [_family, bits, is_cidr, length, address @ ..] = bytes else {
        return None;
    };
    if usize::from(*length) != address.len() {
        return None;
    }
    let (address, full) = match address.len() {
        4 => (
            std::net::Ipv4Addr::from(fixed::<4>(address)?).to_string(),
            32,
        ),
        16 => (
            ipv6_text(std::net::Ipv6Addr::from(fixed::<16>(address)?)),
            128,
        ),
        _ => return None,
    };
    Some(if *is_cidr != 0 || *bits != full {
        format!("{address}/{bits}")
    } else {
        address
    })
}

/// An IPv6 address as PostgreSQL prints one, which is how Rust does, except
/// that it writes an address whose first six groups alone are zero with its
/// last 32 bits as an IPv4 address.
fn ipv6_text(address: std::net::Ipv6Addr) -> String {
    let segments = address.segments();
    if segments[..6] == [0; 6] && segments[6] != 0 {
        let [.., a, b, c, d] = address.octets();
        return format!("::{}", std::net::Ipv4Addr::new(a, b, c, d));
    }
    address.to_string()
}
