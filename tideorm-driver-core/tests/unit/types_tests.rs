use super::*;

/// A decimal's text is read whole: a value needing more digits than `Decimal` holds is refused
/// instead of rounded, while zeros past those digits are dropped.
#[test]
fn a_decimal_is_read_from_text_exactly_or_refused() {
    let read = |text: &str| decimal_from_text(text).map_err(|e| e.to_string());
    let zeros = "0".repeat(30);
    for (text, expected) in [
        ("-123.45".into(), Decimal::new(-12345, 2)),
        (format!("1.5{}", &zeros[1..]), Decimal::new(15, 1)),
        (format!("-0.{zeros}"), Decimal::ZERO),
        (
            format!("9223372036854775807.{zeros}"),
            Decimal::from(i64::MAX),
        ),
        ("79228162514264337593543950335".into(), Decimal::MAX),
    ] {
        assert_eq!(read(&text), Ok(expected), "{text}");
    }
    for text in [
        "0.123456789012345678901234567890",
        "7922816251426433759354395033.55",
        "79228162514264337593543950336",
        "1.2.3",
    ] {
        assert!(read(text).is_err(), "{text}");
    }
}
