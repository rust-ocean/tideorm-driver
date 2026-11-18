use super::{Decimal, PgNumeric, PgNumericSign};
use std::convert::TryFrom;

use PgNumericSign::{Negative, Positive};

/// A decimal, and the numeric it encodes to: sign, scale, weight and base-10000 digits.
const CASES: &[(&str, PgNumericSign, i16, i16, &[i16])] = &[
    ("0", Positive, 0, 0, &[]),
    ("1", Positive, 0, 0, &[1]),
    ("10", Positive, 0, 0, &[10]),
    ("100", Positive, 0, 0, &[100]),
    // Decimal doesn't normalize here
    ("10000", Positive, 0, 1, &[1]),
    ("12345", Positive, 0, 1, &[1, 2345]),
    ("0.1", Positive, 1, -1, &[1000]),
    ("1.2345", Positive, 4, 0, &[1, 2345]),
    ("0.12345", Positive, 5, -1, &[1234, 5000]),
    ("0.01234", Positive, 5, -1, &[123, 4000]),
    ("12345.67890", Positive, 5, 1, &[1, 2345, 6789]),
    ("0.00001234", Positive, 8, -2, &[1234]),
    // `Decimal::MAX`, then the same mantissa at the largest scale
    (
        "79228162514264337593543950335",
        Positive,
        0,
        7,
        &[7, 9228, 1625, 1426, 4337, 5935, 4395, 335],
    ),
    (
        "7.9228162514264337593543950335",
        Positive,
        28,
        0,
        &[7, 9228, 1625, 1426, 4337, 5935, 4395, 335],
    ),
    // Four-digit groups (upstream issue 423)
    ("1234", Positive, 0, 0, &[1234]),
    ("-1234", Negative, 0, 0, &[1234]),
    ("12345678", Positive, 0, 1, &[1234, 5678]),
    ("-12345678", Negative, 0, 1, &[1234, 5678]),
    // Trailing zeros keep their scale (upstream issue 2247)
    ("100.00", Positive, 2, 0, &[100]),
];

#[test]
fn decimals_encode_to_numerics_and_decode_back_unchanged() {
    for &(text, sign, scale, weight, digits) in CASES {
        let decimal: Decimal = text.parse().unwrap();
        let numeric = PgNumeric::Number {
            sign,
            scale,
            weight,
            digits: digits.to_vec(),
        };
        assert_eq!(PgNumeric::from(&decimal), numeric, "{text}");

        let decoded = Decimal::try_from(&numeric).unwrap();
        assert_eq!(
            (decoded.mantissa(), decoded.scale()),
            (decimal.mantissa(), decimal.scale()),
            "{text}"
        );
    }
    assert_eq!(PgNumeric::from(&Decimal::ZERO), PgNumeric::ZERO);
    // The `Decimal::MAX` row above is that constant.
    assert_eq!(
        "79228162514264337593543950335".parse::<Decimal>().unwrap(),
        Decimal::MAX
    );
}

#[test]
fn mult_overflow() {
    //  -24_0711_6702_1036_7100_2022_8579_3280.00
    let large_negative_number = PgNumeric::Number {
        sign: PgNumericSign::Negative,
        scale: 0,
        weight: 7,
        digits: vec![24, 711, 6702, 1036, 7100, 2022, 8579, 3280],
    };

    assert!(Decimal::try_from(large_negative_number).is_err());
}

/// A NUMERIC shorter than its header, or than the digits its header counts, is
/// an error.
#[test]
fn truncated_numerics_are_errors() {
    let one = [0, 1, 0, 0, 0, 0, 0, 0, 0, 1];
    assert_eq!(
        PgNumeric::decode(&one).unwrap(),
        PgNumeric::Number {
            sign: Positive,
            digits: vec![1],
            weight: 0,
            scale: 0,
        }
    );
    for len in [0, 3, 8, 9] {
        assert!(PgNumeric::decode(&one[..len]).is_err());
    }
}

/// A numeric with more digits than `Decimal` holds is refused, not rounded, in either format;
/// the zeros a declared scale past 28 adds are not digits it needs.
#[test]
fn numerics_too_precise_for_decimal_are_refused() {
    use crate::decode::Decode;
    use crate::test_support::text;
    use crate::PgTypeInfo;

    let number = |digits: &[i16], weight, scale| PgNumeric::Number {
        sign: Positive,
        digits: digits.to_vec(),
        weight,
        scale,
    };
    // 1.5 in a numeric(40, 30), and 0.123456789012345678901234567890
    assert_eq!(
        Decimal::try_from(&number(&[1, 5000], 0, 30)).unwrap(),
        Decimal::new(15, 1)
    );
    let long = [1234, 5678, 9012, 3456, 7890, 1234, 5678, 9000];
    assert!(Decimal::try_from(&number(&long, -1, 30)).is_err());
    // 9234567890123456789012345678.9: past `Decimal::MAX` once its fraction is kept
    let wide = [9234, 5678, 9012, 3456, 7890, 1234, 5678, 9000];
    assert!(Decimal::try_from(&number(&wide, 6, 1)).is_err());

    let read = |value: &str| Decimal::decode(text(value, PgTypeInfo::NUMERIC));
    assert_eq!(
        read("1.500000000000000000000000000000").unwrap(),
        Decimal::new(15, 1)
    );
    assert!(read("0.123456789012345678901234567890").is_err());
    assert!(read("9234567890123456789012345678.9").is_err());
    assert_eq!(
        read("1234567890123456789012345678.9").unwrap().to_string(),
        "1234567890123456789012345678.9"
    );
}
