use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};

use super::MICROS_PER_DAY;
use crate::decode::Decode;
use crate::test_support::binary;
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
