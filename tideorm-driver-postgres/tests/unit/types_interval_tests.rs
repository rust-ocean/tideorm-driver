use super::*;

fn interval(months: i32, days: i32, microseconds: i64) -> PgInterval {
    PgInterval {
        months,
        days,
        microseconds,
    }
}

/// An `INTERVAL` is sent as its microseconds (8 bytes), days and months (4 each), big-endian.
#[test]
fn interval_round_trips_binary() {
    let cases = [
        (interval(0, 0, 0), "0000000000000000 00000000 00000000"),
        (interval(0, 0, 1_000), "00000000000003e8 00000000 00000000"),
        (
            interval(0, 0, 1_000_000),
            "00000000000f4240 00000000 00000000",
        ),
        (
            interval(0, 0, 3_600_000_000),
            "00000000d693a400 00000000 00000000",
        ),
        (interval(0, 1, 0), "0000000000000000 00000001 00000000"),
        (interval(1, 0, 0), "0000000000000000 00000000 00000001"),
    ];

    for (value, hex) in cases {
        let expected = hex::decode(hex.replace(' ', "")).unwrap();

        let mut buf = PgArgumentBuffer::default();
        assert!(matches!(
            Encode::<Postgres>::encode(value, &mut buf),
            Ok(IsNull::No)
        ));
        assert_eq!(*buf, expected, "{value:?}");
        assert_eq!(PgInterval::from_binary(&expected).unwrap(), value);
    }

    assert_eq!(PgInterval::default(), interval(0, 0, 0));
}

/// A duration converts to microseconds alone; one with a fraction of a microsecond, or too long
/// for an `i64` of them, is refused.
#[test]
fn interval_from_std_duration() {
    use std::time::Duration;

    let cases = [
        (Duration::from_micros(27_000), Some(27_000)),
        (Duration::from_nanos(27_000_001), None),
        (Duration::from_secs(20_000_000_000_000), None),
    ];

    for (duration, microseconds) in cases {
        let converted = PgInterval::try_from(duration).ok();
        assert_eq!(
            converted,
            microseconds.map(|us| interval(0, 0, us)),
            "{duration:?}"
        );
    }
}

#[test]
fn interval_from_chrono_duration() {
    use chrono::Duration;

    let cases = [
        (Duration::microseconds(27_000), Some(27_000)),
        (Duration::microseconds(-27_000), Some(-27_000)),
        (Duration::nanoseconds(27_000_001), None),
        (Duration::nanoseconds(-27_000_001), None),
        (Duration::seconds(10_000_000_000), None),
        (Duration::seconds(-10_000_000_000), None),
    ];

    for (duration, microseconds) in cases {
        let converted = PgInterval::try_from(duration).ok();
        assert_eq!(
            converted,
            microseconds.map(|us| interval(0, 0, us)),
            "{duration:?}"
        );
    }
}
