//! Reading a `MySqlTime` from the text protocol.

use driver_core::error::BoxDynError;

use super::{MySqlTime, MySqlTimeSign};

// Not exposing this as a `FromStr` impl currently because `MySqlTime` is not designed to be
// a general interchange type.
pub(super) fn parse(text: &str) -> Result<MySqlTime, BoxDynError> {
    let mut segments = text.split(':');

    let hours = segments
        .next()
        .ok_or("expected hours segment, got nothing")?;

    let minutes = segments
        .next()
        .ok_or("expected minutes segment, got nothing")?;

    let seconds = segments
        .next()
        .ok_or("expected seconds segment, got nothing")?;

    // The sign comes from the text: a negative time under an hour reads
    // `-00:..`, whose hours parse as zero.
    let sign = if hours.starts_with('-') {
        MySqlTimeSign::Negative
    } else {
        MySqlTimeSign::Positive
    };

    // The allowed range of whole hours is much smaller than `i32`'s positive range.
    let hours = hours
        .parse::<i32>()
        .map_err(|e| format!("error parsing hours from {text:?} (segment {hours:?}): {e}"))?
        .unsigned_abs();

    let minutes: u8 = minutes
        .parse()
        .map_err(|e| format!("error parsing minutes from {text:?} (segment {minutes:?}): {e}"))?;

    let (seconds, microseconds): (u8, u32) = if let Some((seconds, microseconds)) =
        seconds.split_once('.')
    {
        (
            seconds.parse().map_err(|e| {
                format!("error parsing seconds from {text:?} (segment {seconds:?}): {e}")
            })?,
            parse_microseconds(microseconds).map_err(|e| {
                format!("error parsing microseconds from {text:?} (segment {microseconds:?}): {e}")
            })?,
        )
    } else {
        (
            seconds.parse().map_err(|e| {
                format!("error parsing seconds from {text:?} (segment {seconds:?}): {e}")
            })?,
            0,
        )
    };

    Ok(MySqlTime::new(sign, hours, minutes, seconds, microseconds)?)
}

/// Parse microseconds from a fractional seconds string.
pub(super) fn parse_microseconds(micros: &str) -> Result<u32, BoxDynError> {
    const EXPECTED_DIGITS: usize = 6;

    match micros.len() {
        0 => Err("empty string".into()),
        len @ ..=EXPECTED_DIGITS => {
            // Fewer than 6 digits, multiply to the correct magnitude
            let micros: u32 = micros.parse()?;
            // cast cannot overflow
            #[allow(clippy::cast_possible_truncation)]
            Ok(micros * 10u32.pow((EXPECTED_DIGITS - len) as u32))
        }
        // More digits than expected, truncate
        _ => Ok(micros[..EXPECTED_DIGITS].parse()?),
    }
}
