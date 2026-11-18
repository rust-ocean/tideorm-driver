use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};

use super::MICROS_PER_DAY;
use crate::decode::Decode;
use crate::test_support::{binary, text};
use crate::types::{PgTimeTz, PgUnrepresentableMoment};
use crate::{PgTypeInfo, Postgres};

fn decode_error<T: for<'r> Decode<'r, Postgres>>(bytes: &[u8], type_info: PgTypeInfo) -> String {
    match T::decode(binary(bytes, type_info)) {
        Ok(_) => panic!("decoding {bytes:?} should fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn infinite_dates_are_errors() {
    let infinity = i32::MAX.to_be_bytes();
    let minus_infinity = i32::MIN.to_be_bytes();

    assert_eq!(
        decode_error::<NaiveDate>(&infinity, PgTypeInfo::DATE),
        "DATE 'infinity' has no Rust date or time equivalent"
    );
    assert_eq!(
        decode_error::<NaiveDate>(&minus_infinity, PgTypeInfo::DATE),
        "DATE '-infinity' has no Rust date or time equivalent"
    );
    let error = NaiveDate::decode(binary(&infinity, PgTypeInfo::DATE)).unwrap_err();
    assert_eq!(
        error.downcast_ref::<PgUnrepresentableMoment>(),
        Some(&PgUnrepresentableMoment {
            type_name: "DATE",
            value: "infinity"
        })
    );
}

#[test]
fn infinite_timestamps_are_errors() {
    let infinity = i64::MAX.to_be_bytes();
    let minus_infinity = i64::MIN.to_be_bytes();

    assert_eq!(
        decode_error::<NaiveDateTime>(&infinity, PgTypeInfo::TIMESTAMP),
        "TIMESTAMP 'infinity' has no Rust date or time equivalent"
    );
    assert_eq!(
        decode_error::<DateTime<Utc>>(&minus_infinity, PgTypeInfo::TIMESTAMPTZ),
        "TIMESTAMPTZ '-infinity' has no Rust date or time equivalent"
    );
    assert_eq!(
        decode_error::<NaiveDateTime>(&(i64::MAX - 1).to_be_bytes(), PgTypeInfo::TIMESTAMP),
        format!("timestamp value {} is outside chrono's range", i64::MAX - 1)
    );
}

#[test]
fn finite_dates_and_timestamps_still_decode() {
    let date = NaiveDate::decode(binary(&(-1i32).to_be_bytes(), PgTypeInfo::DATE)).unwrap();
    assert_eq!(date, NaiveDate::from_ymd_opt(1999, 12, 31).unwrap());

    let micros = 1_500_000i64.to_be_bytes();
    let timestamp = NaiveDateTime::decode(binary(&micros, PgTypeInfo::TIMESTAMP)).unwrap();
    assert_eq!(
        timestamp,
        NaiveDate::from_ymd_opt(2000, 1, 1)
            .unwrap()
            .and_hms_milli_opt(0, 0, 1, 500)
            .unwrap()
    );
}

#[test]
fn time_at_twenty_four_hours_is_an_error() {
    let end_of_day = MICROS_PER_DAY.to_be_bytes();

    assert_eq!(
        decode_error::<NaiveTime>(&end_of_day, PgTypeInfo::TIME),
        "TIME '24:00:00' has no Rust date or time equivalent"
    );
    assert_eq!(
        decode_error::<NaiveTime>(&(-1i64).to_be_bytes(), PgTypeInfo::TIME),
        "TIME of -1 microseconds after midnight is out of range"
    );

    let mut with_offset = end_of_day.to_vec();
    with_offset.extend_from_slice(&0i32.to_be_bytes());
    assert_eq!(
        decode_error::<PgTimeTz<NaiveTime, FixedOffset>>(&with_offset, PgTypeInfo::TIMETZ),
        "TIME '24:00:00' has no `NaiveTime` equivalent"
    );
}

#[test]
fn the_last_microsecond_of_the_day_still_decodes() {
    let micros = (MICROS_PER_DAY - 1).to_be_bytes();
    let time = NaiveTime::decode(binary(&micros, PgTypeInfo::TIME)).unwrap();

    assert_eq!(
        time,
        NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
    );
}

/// Dates and timestamps read from text as PostgreSQL prints them: before year 1 (` BC`), past
/// year 9999, and in a time zone whose offset has seconds (local mean time before 1883).
#[test]
fn dates_and_timestamps_read_from_text_in_every_form() {
    let date =
        |text_value: &str| NaiveDate::decode(text(text_value, PgTypeInfo::DATE)).expect(text_value);
    assert_eq!(
        date("2024-03-10"),
        NaiveDate::from_ymd_opt(2024, 3, 10).unwrap()
    );
    assert_eq!(
        date("0044-03-15 BC"),
        NaiveDate::from_ymd_opt(-43, 3, 15).unwrap()
    );
    assert_eq!(
        date("12000-01-01"),
        NaiveDate::from_ymd_opt(12000, 1, 1).unwrap()
    );

    let stamp = |text_value: &str| {
        NaiveDateTime::decode(text(text_value, PgTypeInfo::TIMESTAMP)).expect(text_value)
    };
    let at = |y, m, d, h, min, s, us| {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_micro_opt(h, min, s, us)
            .unwrap()
    };
    assert_eq!(
        stamp("2024-06-15 12:34:56.123456"),
        at(2024, 6, 15, 12, 34, 56, 123_456)
    );
    assert_eq!(stamp("0001-01-01 00:00:00 BC"), at(0, 1, 1, 0, 0, 0, 0));
    assert_eq!(
        stamp("262142-12-31 23:59:59"),
        at(262_142, 12, 31, 23, 59, 59, 0)
    );

    let zoned = |text_value: &str| {
        let value = text(text_value, PgTypeInfo::TIMESTAMPTZ);
        let fixed = DateTime::<FixedOffset>::decode(value).expect(text_value);
        let utc = DateTime::<Utc>::decode(text(text_value, PgTypeInfo::TIMESTAMPTZ)).unwrap();
        assert_eq!(fixed, utc, "{text_value}");
        (fixed.offset().local_minus_utc(), utc.naive_utc())
    };
    assert_eq!(
        zoned("2024-03-10 08:00:00+05:30"),
        (19_800, at(2024, 3, 10, 2, 30, 0, 0))
    );
    assert_eq!(
        zoned("1850-01-01 00:00:00-04:56:02"),
        (-17_762, at(1850, 1, 1, 4, 56, 2, 0))
    );
    assert_eq!(
        zoned("0044-03-15 12:00:00+00 BC"),
        (0, at(-43, 3, 15, 12, 0, 0, 0))
    );
    assert_eq!(
        zoned("2024-11-03 01:30:00-04"),
        (-14_400, at(2024, 11, 3, 5, 30, 0, 0))
    );

    // a year past chrono's range is an error, as in binary
    for bad in [
        "294276-12-31 23:59:59",
        "2024-13-01",
        "2024-01-01 BC BC",
        "abc",
        "2024-01-01 25:00:00",
    ] {
        assert!(
            NaiveDateTime::decode(text(bad, PgTypeInfo::TIMESTAMP)).is_err(),
            "{bad}"
        );
    }
}

/// A `TIMETZ` reads from text with an offset of seconds, as it does from binary.
#[test]
fn a_time_with_an_offset_of_seconds_reads_from_text() {
    let read = |text_value: &str| {
        PgTimeTz::<NaiveTime, FixedOffset>::decode(text(text_value, PgTypeInfo::TIMETZ))
            .expect(text_value)
    };
    let noon = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    let east = |seconds| FixedOffset::east_opt(seconds).unwrap();
    for (text_value, offset) in [
        ("12:00:00-04:56:02", -17_762),
        ("12:00:00+05:30", 19_800),
        ("12:00:00+14", 50_400),
    ] {
        assert_eq!(
            read(text_value),
            PgTimeTz {
                time: noon,
                offset: east(offset)
            }
        );
    }
}
