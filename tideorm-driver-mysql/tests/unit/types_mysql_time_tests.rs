use super::MySqlTime;
use crate::types::MySqlTimeSign;

use super::text::parse_microseconds;

#[test]
fn test_display() {
    assert_eq!(MySqlTime::ZERO.to_string(), "0:00:00");

    assert_eq!(format!("{:.0}", MySqlTime::ZERO), "0:00:00");

    assert_eq!(format!("{:.3}", MySqlTime::ZERO), "0:00:00.000");

    assert_eq!(format!("{:.6}", MySqlTime::ZERO), "0:00:00.000000");

    assert_eq!(format!("{:.9}", MySqlTime::ZERO), "0:00:00.000000000");

    assert_eq!(format!("{:.0}", MySqlTime::MAX), "838:59:59");

    assert_eq!(format!("{:.3}", MySqlTime::MAX), "838:59:59.000");

    assert_eq!(format!("{:.6}", MySqlTime::MAX), "838:59:59.000000");

    assert_eq!(format!("{:.9}", MySqlTime::MAX), "838:59:59.000000000");

    assert_eq!(format!("{:+.0}", MySqlTime::MAX), "+838:59:59");

    assert_eq!(format!("{:+.3}", MySqlTime::MAX), "+838:59:59.000");

    assert_eq!(format!("{:+.6}", MySqlTime::MAX), "+838:59:59.000000");

    assert_eq!(format!("{:+.9}", MySqlTime::MAX), "+838:59:59.000000000");

    assert_eq!(format!("{:.0}", MySqlTime::MIN), "-838:59:59");

    assert_eq!(format!("{:.3}", MySqlTime::MIN), "-838:59:59.000");

    assert_eq!(format!("{:.6}", MySqlTime::MIN), "-838:59:59.000000");

    assert_eq!(format!("{:.9}", MySqlTime::MIN), "-838:59:59.000000000");

    let positive = MySqlTime::new(MySqlTimeSign::Positive, 123, 45, 56, 890011).unwrap();

    assert_eq!(positive.to_string(), "123:45:56.890011");
    assert_eq!(format!("{positive:.0}"), "123:45:56");
    assert_eq!(format!("{positive:.3}"), "123:45:56.890");
    assert_eq!(format!("{positive:.6}"), "123:45:56.890011");
    assert_eq!(format!("{positive:.9}"), "123:45:56.890011000");

    assert_eq!(format!("{positive:+.0}"), "+123:45:56");
    assert_eq!(format!("{positive:+.3}"), "+123:45:56.890");
    assert_eq!(format!("{positive:+.6}"), "+123:45:56.890011");
    assert_eq!(format!("{positive:+.9}"), "+123:45:56.890011000");

    let negative = MySqlTime::new(MySqlTimeSign::Negative, 123, 45, 56, 890011).unwrap();

    assert_eq!(negative.to_string(), "-123:45:56.890011");
    assert_eq!(format!("{negative:.0}"), "-123:45:56");
    assert_eq!(format!("{negative:.3}"), "-123:45:56.890");
    assert_eq!(format!("{negative:.6}"), "-123:45:56.890011");
    assert_eq!(format!("{negative:.9}"), "-123:45:56.890011000");
}

#[test]
fn test_parse_microseconds() {
    assert_eq!(parse_microseconds("010").unwrap(), 10_000);

    assert_eq!(parse_microseconds("0100000000").unwrap(), 10_000);

    assert_eq!(parse_microseconds("890").unwrap(), 890_000);

    assert_eq!(parse_microseconds("0890").unwrap(), 89_000);

    assert_eq!(
        // Case in point about not exposing this:
        // we always truncate excess precision because it's simpler than rounding
        // and MySQL should never return a higher precision.
        parse_microseconds("123456789").unwrap(),
        123456,
    );
}

#[test]
fn test_sign_predicates() {
    let negative = MySqlTime::new(MySqlTimeSign::Negative, 1, 0, 0, 0).unwrap();
    assert!(negative.is_negative());
    assert!(!negative.is_positive());

    let positive = MySqlTime::new(MySqlTimeSign::Positive, 1, 0, 0, 0).unwrap();
    assert!(!positive.is_negative());
    assert!(positive.is_positive());
    assert!(!MySqlTime::ZERO.is_negative());
}

#[test]
fn test_naive_time_refuses_durations() {
    use chrono::NaiveTime;

    let time_of_day = MySqlTime::new(MySqlTimeSign::Positive, 23, 59, 59, 999_999).unwrap();
    assert_eq!(
        NaiveTime::try_from(time_of_day).unwrap(),
        NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap()
    );

    let negative = MySqlTime::new(MySqlTimeSign::Negative, 1, 2, 3, 0).unwrap();
    assert!(NaiveTime::try_from(negative).is_err());

    let day = MySqlTime::new(MySqlTimeSign::Positive, 24, 0, 0, 0).unwrap();
    assert!(NaiveTime::try_from(day).is_err());
}

#[test]
fn test_parse_keeps_the_sign_under_an_hour() {
    let time = super::text::parse("-00:30:15.5").unwrap();
    assert!(time.is_negative());
    assert_eq!(time.to_string(), "-0:30:15.5");

    let time = super::text::parse("12:34:56").unwrap();
    assert!(time.is_positive());
    assert_eq!(time.hours(), 12);
}
