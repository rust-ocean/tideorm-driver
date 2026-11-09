use super::numeric::numeric_text;
use super::*;
use crate::type_info::PgCustomType;
use crate::types::Oid;
use std::sync::Arc;
use PgDecodedValue as V;

fn raw(ty: PgTypeInfo, bytes: Option<&[u8]>, format: PgValueFormat) -> PgValueRef<'_> {
    PgValueRef {
        value: bytes,
        row: None,
        type_info: ty,
        format,
    }
}

/// `bytes` decoded as a binary value of `ty`, or `None` when they are not one.
fn binary(ty: PgTypeInfo, bytes: &[u8]) -> Option<PgDecodedValue> {
    raw(ty, Some(bytes), PgValueFormat::Binary)
        .decode_lossless()
        .ok()
}

/// The text a binary value of `ty` decodes to.
fn text(ty: PgTypeInfo, bytes: &[u8]) -> String {
    match binary(ty.clone(), bytes) {
        Some(V::Text(text)) => text,
        other => panic!("{ty}: {other:?}"),
    }
}

fn custom(kind: PgTypeKind) -> PgTypeInfo {
    PgTypeInfo(PgType::Custom(Arc::new(PgCustomType {
        oid: Oid(90000),
        name: "custom".into(),
        kind,
    })))
}

fn numeric(ndigits: u16, weight: i16, sign: u16, scale: u16, digits: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for word in [
        ndigits,
        u16::from_be_bytes(weight.to_be_bytes()),
        sign,
        scale,
    ] {
        bytes.extend(word.to_be_bytes());
    }
    for digit in digits {
        bytes.extend(digit.to_be_bytes());
    }
    bytes
}

/// 2020-01-01, in days and in microseconds since PostgreSQL's epoch.
const NEW_YEAR_2020: i32 = 7305;
const NEW_YEAR_2020_AT_TEN: i64 = 7305 * 86_400_000_000 + 10 * 3_600_000_000;

fn new_year_2020_at_ten() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2020, 1, 1)
        .and_then(|date| date.and_hms_opt(10, 0, 0))
        .unwrap()
}

fn interval(micros: i64, days: i32, months: i32) -> Vec<u8> {
    [
        &micros.to_be_bytes()[..],
        &days.to_be_bytes(),
        &months.to_be_bytes(),
    ]
    .concat()
}

/// A length-prefixed value, or a `NULL`.
fn value(bytes: Option<&[u8]>) -> Vec<u8> {
    match bytes {
        Some(bytes) => [
            &(i32::try_from(bytes.len()).unwrap()).to_be_bytes()[..],
            bytes,
        ]
        .concat(),
        None => (-1_i32).to_be_bytes().to_vec(),
    }
}

fn array(lengths: &[i32], elements: &[Option<&[u8]>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for word in [i32::try_from(lengths.len()).unwrap(), 1, 0] {
        bytes.extend(word.to_be_bytes());
    }
    for length in lengths {
        bytes.extend(length.to_be_bytes());
        bytes.extend(1_i32.to_be_bytes());
    }
    for element in elements {
        bytes.extend(value(*element));
    }
    bytes
}

fn range(flags: u8, bounds: &[&[u8]]) -> Vec<u8> {
    let mut bytes = vec![flags];
    for bound in bounds {
        bytes.extend(value(Some(bound)));
    }
    bytes
}

#[test]
fn numeric_text_reads_the_binary_form_exactly() {
    let thirty_digits = numeric(8, 7, 0, 0, &[12, 3456, 7890, 1234, 5678, 9012, 3456, 7890]);
    assert_eq!(
        numeric_text(&thirty_digits).as_deref(),
        Some("123456789012345678901234567890")
    );
    assert_eq!(
        binary(PgTypeInfo::NUMERIC, &thirty_digits),
        Some(V::Numeric("123456789012345678901234567890".into()))
    );
    assert_eq!(
        numeric_text(&thirty_digits[..thirty_digits.len() - 1]),
        None
    );
    assert_eq!(
        numeric_text(&numeric(2, 0, 0x4000, 1, &[1, 5000])).as_deref(),
        Some("-1.5")
    );
    assert_eq!(
        numeric_text(&numeric(1, -1, 0, 3, &[500])).as_deref(),
        Some("0.050")
    );
    assert_eq!(
        numeric_text(&numeric(0, 0, 0, 0, &[])).as_deref(),
        Some("0")
    );
    for (sign, expected) in [(0xC000, "NaN"), (0xD000, "Infinity"), (0xF000, "-Infinity")] {
        assert_eq!(
            numeric_text(&numeric(0, 0, sign, 0, &[])).as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn intervals_read_as_postgres_writes_iso_8601() {
    let four_five_six = (4 * 3600 + 5 * 60 + 6) * 1_000_000;
    for (micros, days, months, expected) in [
        (0, 0, 0, "PT0S"),
        (four_five_six + 789_000, 3, 14, "P1Y2M3DT4H5M6.789S"),
        (-four_five_six, 3, -14, "P-1Y-2M3DT-4H-5M-6S"),
        (-1, 0, 0, "PT-0.000001S"),
        (1_500_000, 0, 0, "PT1.5S"),
        (100 * 3_600_000_000, 0, 0, "PT100H"),
        (0, -1, 1, "P1M-1D"),
        (i64::MAX, i32::MAX, i32::MAX, "infinity"),
        (i64::MIN, i32::MIN, i32::MIN, "-infinity"),
    ] {
        assert_eq!(
            text(PgTypeInfo::INTERVAL, &interval(micros, days, months)),
            expected
        );
    }
    assert_eq!(binary(PgTypeInfo::INTERVAL, &[0; 15]), None);
}

#[test]
fn addresses_read_as_postgres_prints_them() {
    let ipv6 = |segments: [u16; 8]| std::net::Ipv6Addr::from(segments).octets();
    for (bytes, expected) in [
        (
            [&[2, 32, 0, 4][..], &[192, 168, 1, 5]].concat(),
            "192.168.1.5",
        ),
        (
            [&[2, 24, 0, 4][..], &[192, 168, 1, 0]].concat(),
            "192.168.1.0/24",
        ),
        (
            [&[2, 32, 1, 4][..], &[192, 168, 1, 5]].concat(),
            "192.168.1.5/32",
        ),
        (
            [&[3, 128, 0, 16][..], &ipv6([0, 0, 0, 0, 0, 0, 0, 1])].concat(),
            "::1",
        ),
        (
            [
                &[3, 128, 0, 16][..],
                &ipv6([0, 0, 0, 0, 0, 0, 0x0102, 0x0304]),
            ]
            .concat(),
            "::1.2.3.4",
        ),
        (
            [
                &[3, 128, 0, 16][..],
                &ipv6([0, 0, 0, 0, 0, 0xffff, 0x0102, 0x0304]),
            ]
            .concat(),
            "::ffff:1.2.3.4",
        ),
        (
            [
                &[3, 64, 0, 16][..],
                &ipv6([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1]),
            ]
            .concat(),
            "2001:db8::1/64",
        ),
    ] {
        assert_eq!(text(PgTypeInfo(PgType::Inet), &bytes), expected);
    }
}

#[test]
fn zoned_times_and_money_read_exactly() {
    let timetz = |micros: i64, west: i32| [&micros.to_be_bytes()[..], &west.to_be_bytes()].concat();
    for (micros, west, expected) in [
        (37_800_000_000, -7_200, "10:30:00+02"),
        (37_800_250_000, 19_800, "10:30:00.25-05:30"),
        (43_200_000_000, -19_815, "12:00:00+05:30:15"),
        (86_400_000_000, 3_600, "24:00:00-01"),
    ] {
        assert_eq!(text(PgTypeInfo::TIMETZ, &timetz(micros, west)), expected);
    }
    for cents in [123_456_i64, -1, i64::MAX] {
        assert_eq!(
            binary(PgTypeInfo::MONEY, &cents.to_be_bytes()),
            Some(V::Money(PgMoney(cents)))
        );
    }
}

#[test]
fn ranges_read_as_postgres_prints_them() {
    let int4 = |n: i32| n.to_be_bytes();
    assert_eq!(
        text(PgTypeInfo::INT4_RANGE, &range(0x02, &[&int4(1), &int4(10)])),
        "[1,10)"
    );
    assert_eq!(text(PgTypeInfo::INT4_RANGE, &range(0x01, &[])), "empty");
    assert_eq!(
        text(PgTypeInfo::INT4_RANGE, &range(0x08, &[&int4(5)])),
        "(,5)"
    );
    assert_eq!(text(PgTypeInfo::INT8_RANGE, &range(0x18, &[])), "(,)");
    assert_eq!(
        text(
            PgTypeInfo::NUM_RANGE,
            &range(
                0x06,
                &[&numeric(2, 0, 0, 1, &[1, 5000]), &numeric(1, 0, 0, 0, &[2])]
            )
        ),
        "[1.5,2]"
    );
    assert_eq!(
        text(
            PgTypeInfo::DATE_RANGE,
            &range(
                0x02,
                &[&NEW_YEAR_2020.to_be_bytes(), &i32::MAX.to_be_bytes()]
            )
        ),
        "[2020-01-01,infinity)"
    );
    let ten = NEW_YEAR_2020_AT_TEN.to_be_bytes();
    let half_past = (NEW_YEAR_2020_AT_TEN + 1_800_500_000).to_be_bytes();
    assert_eq!(
        text(PgTypeInfo::TS_RANGE, &range(0x02, &[&ten, &half_past])),
        "[\"2020-01-01 10:00:00\",\"2020-01-01 10:30:00.5\")"
    );
    assert_eq!(
        text(PgTypeInfo::TSTZ_RANGE, &range(0x10 | 0x02, &[&ten])),
        "[\"2020-01-01 10:00:00+00\",)"
    );
    // A date before year 1 is ` BC`, after the zone.
    let one_bc = (-730_485_i64 * 86_400_000_000).to_be_bytes();
    assert_eq!(
        text(
            PgTypeInfo::TSTZ_RANGE,
            &range(0x02 | 0x04, &[&one_bc, &one_bc])
        ),
        "[\"0001-01-01 00:00:00+00 BC\",\"0001-01-01 00:00:00+00 BC\"]"
    );
}

#[test]
fn multiranges_use_resolved_subtypes_and_reject_invalid_frames() {
    let int4_range = |a: i32, b: i32| range(0x02, &[&a.to_be_bytes(), &b.to_be_bytes()]);
    let mut bytes = 2_i32.to_be_bytes().to_vec();
    for r in [int4_range(1, 3), int4_range(5, 7)] {
        bytes.extend(value(Some(&r)));
    }
    let ty = custom(PgTypeKind::Multirange(PgTypeInfo::INT4));
    assert_eq!(
        binary(ty.clone(), &bytes),
        Some(V::Text("{[1,3),[5,7)}".into()))
    );
    assert_eq!(binary(ty.clone(), &bytes[..bytes.len() - 1]), None);
    assert_eq!(
        binary(ty.clone(), &0_i32.to_be_bytes()),
        Some(V::Text("{}".into()))
    );
    assert_eq!(
        raw(ty.clone(), None, PgValueFormat::Binary)
            .decode_lossless()
            .unwrap(),
        V::Null
    );
    assert_eq!(
        raw(ty, Some(b"{[1,3)}"), PgValueFormat::Text)
            .decode_lossless()
            .unwrap(),
        V::Text("{[1,3)}".into())
    );
}

#[test]
fn arrays_keep_their_shape_nulls_and_infinite_moments() {
    let int4 = |n: i32| n.to_be_bytes();
    assert_eq!(
        binary(
            PgTypeInfo::INT4_ARRAY,
            &array(
                &[2, 2],
                &[Some(&int4(1)), None, Some(&int4(3)), Some(&int4(4))]
            )
        ),
        Some(V::Array(PgDecodedArray {
            dimensions: vec![2, 2],
            elements: vec![V::Integer(1), V::Null, V::Integer(3), V::Integer(4)],
        }))
    );
    assert_eq!(
        binary(PgTypeInfo::INT4_ARRAY, &array(&[], &[])),
        Some(V::Array(PgDecodedArray {
            dimensions: vec![],
            elements: vec![],
        }))
    );
    let elements = |ty: PgTypeInfo, values: &[Option<&[u8]>]| match binary(
        ty,
        &array(&[i32::try_from(values.len()).unwrap()], values),
    ) {
        Some(V::Array(array)) => array.elements,
        other => panic!("{other:?}"),
    };
    let at_ten = new_year_2020_at_ten();
    assert_eq!(
        elements(
            PgTypeInfo::DATE_ARRAY,
            &[
                Some(&NEW_YEAR_2020.to_be_bytes()),
                None,
                Some(&i32::MIN.to_be_bytes())
            ]
        ),
        [
            V::Date(at_ten.date()),
            V::Null,
            V::SpecialMoment("-infinity")
        ]
    );
    let ten = NEW_YEAR_2020_AT_TEN.to_be_bytes();
    assert_eq!(
        elements(PgTypeInfo::TIMESTAMPTZ_ARRAY, &[Some(&ten)]),
        [V::TimestampWithTimeZone(at_ten.and_utc())]
    );
    assert_eq!(
        elements(PgTypeInfo::TIMESTAMP_ARRAY, &[Some(&ten)]),
        [V::Timestamp(at_ten)]
    );
    assert_eq!(
        elements(
            PgTypeInfo::TIME_ARRAY,
            &[
                Some(&1_500_000_i64.to_be_bytes()),
                Some(&86_400_000_000_i64.to_be_bytes())
            ]
        ),
        [
            V::Time(NaiveTime::from_hms_milli_opt(0, 0, 1, 500).unwrap()),
            V::SpecialMoment("24:00:00")
        ]
    );
    assert_eq!(
        elements(
            PgTypeInfo::INTERVAL_ARRAY,
            &[Some(&interval(0, 1, 0)), None]
        ),
        [V::Text("P1D".into()), V::Null]
    );
    // Bytes that are not an array of the type are no value at all.
    assert_eq!(
        binary(PgTypeInfo::INT4_ARRAY, &array(&[1], &[Some(&[1, 2])])),
        None
    );
    let negative_dimensions: Vec<_> = [-1_i32, 0, 23]
        .into_iter()
        .flat_map(i32::to_be_bytes)
        .collect();
    assert_eq!(binary(PgTypeInfo::INT4_ARRAY, &negative_dimensions), None);
}

#[test]
fn moment_guard_handles_both_protocols_and_domains() {
    for (ty, bytes, text) in [
        (
            PgTypeInfo::DATE,
            i32::MAX.to_be_bytes().to_vec(),
            "infinity",
        ),
        (
            PgTypeInfo::TIMESTAMP,
            i64::MIN.to_be_bytes().to_vec(),
            "-infinity",
        ),
        (
            PgTypeInfo::TIMESTAMPTZ,
            i64::MAX.to_be_bytes().to_vec(),
            "infinity",
        ),
        (
            PgTypeInfo::TIME,
            86_400_000_000_i64.to_be_bytes().to_vec(),
            "24:00:00",
        ),
    ] {
        let domain = custom(PgTypeKind::Domain(ty.clone()));
        let value = raw(ty.clone(), Some(&bytes), PgValueFormat::Binary);
        let moment = value.unrepresentable_moment().unwrap();
        assert_eq!(moment.value(), text);
        assert_eq!(moment.type_name(), ty.to_string());
        assert!(matches!(
            value.decode_lossless().unwrap(),
            V::SpecialMoment(_)
        ));
        assert_eq!(
            raw(ty.clone(), Some(text.as_bytes()), PgValueFormat::Text).unrepresentable_moment(),
            Some(moment)
        );
        assert_eq!(
            raw(ty, None, PgValueFormat::Binary).unrepresentable_moment(),
            None
        );
        assert_eq!(
            raw(domain, Some(&bytes), PgValueFormat::Binary).unrepresentable_moment(),
            Some(moment)
        );
    }
    assert_eq!(
        raw(PgTypeInfo::TEXT, Some(b"infinity"), PgValueFormat::Text).unrepresentable_moment(),
        None
    );
    assert_eq!(
        raw(
            PgTypeInfo::DATE,
            Some(&0_i32.to_be_bytes()),
            PgValueFormat::Binary
        )
        .unrepresentable_moment(),
        None
    );
}

#[test]
fn oids_are_integers_and_unknown_literals_text() {
    assert_eq!(
        binary(PgTypeInfo::OID, &u32::MAX.to_be_bytes()),
        Some(V::Integer(u32::MAX.into()))
    );
    assert_eq!(text(PgTypeInfo::UNKNOWN, b"a\x01"), "a\x01");
}

#[test]
fn enum_labels_and_unknown_binary_values_are_distinguished() {
    let ty = custom(PgTypeKind::Enum(vec!["ready".into()].into()));
    assert_eq!(binary(ty, b"ready"), Some(V::Text("ready".into())));
    assert!(
        raw(PgTypeInfo::POINT, Some(&[0; 16]), PgValueFormat::Binary)
            .decode_lossless()
            .is_err()
    );
}

#[test]
fn unresolved_text_types_do_not_panic_in_the_moment_guard() {
    let value = raw(
        PgTypeInfo::with_oid(Oid(90000)),
        Some(b"ready"),
        PgValueFormat::Text,
    );
    assert_eq!(value.unrepresentable_moment(), None);
    assert_eq!(value.decode_lossless().unwrap(), V::Text("ready".into()));
}
