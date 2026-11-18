//! What a server can send as a column's value: each type's decoder, and the lossless one JSON
//! reads use, takes arbitrary and corrupted bytes in either format and returns an error rather
//! than panicking, allocating no more than those bytes justify.

use super::*;
use crate::decode::Decode;
use crate::test_support::{encoded, fuzz};
use crate::type_info::{PgCustomType, PgType, PgTypeKind};
use crate::{PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use ::chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use ::rust_decimal::Decimal;
use driver_core::types::Json;
use std::ops::Bound;
use std::sync::Arc;

/// Reads `bytes` as a value of `ty` in both formats: losslessly, and as a `T`.
fn read<T: for<'r> Decode<'r, Postgres>>(ty: &PgTypeInfo, bytes: &[u8]) {
    for format in [PgValueFormat::Binary, PgValueFormat::Text] {
        let value = || PgValueRef {
            value: Some(bytes),
            row: None,
            type_info: ty.clone(),
            format,
        };
        let _ = value().decode_lossless();
        let _ = value().unrepresentable_moment();
        let _ = T::decode(value());
    }
}

type Reader = fn(&PgTypeInfo, &[u8]);

fn named(name: &str) -> PgTypeInfo {
    PgTypeInfo(PgType::Custom(Arc::new(PgCustomType {
        oid: Oid(90_000),
        name: name.to_owned().into(),
        kind: PgTypeKind::Simple,
    })))
}

fn floats(values: &[f64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

fn text(samples: &[&str]) -> Vec<Vec<u8>> {
    samples.iter().map(|s| s.as_bytes().to_vec()).collect()
}

/// A binary record of an `INT4` 7 and a `TEXT` "x".
fn record() -> Vec<u8> {
    let mut body = 2_i32.to_be_bytes().to_vec();
    body.extend(23_u32.to_be_bytes());
    body.extend(4_i32.to_be_bytes());
    body.extend(7_i32.to_be_bytes());
    body.extend(25_u32.to_be_bytes());
    body.extend(1_i32.to_be_bytes());
    body.push(b'x');
    body
}

/// Each type, valid values of it in either format, and how to read it.
fn cases() -> Vec<(PgTypeInfo, Vec<Vec<u8>>, Reader)> {
    let at = NaiveDate::from_ymd_opt(2020, 2, 29)
        .unwrap()
        .and_hms_micro_opt(23, 59, 1, 5)
        .unwrap();
    let decimals = [Decimal::new(-123_456_789, 4), Decimal::MAX, Decimal::ZERO];
    let mut numerics: Vec<Vec<u8>> = decimals.iter().map(encoded).collect();
    numerics.extend(text(&["NaN", "-1.5e10", "0.000"]));
    let int_range = PgRange::<i32>::from((Bound::Included(1), Bound::Excluded(9)));
    let date_range = PgRange::<NaiveDate>::from((Bound::Included(at.date()), Bound::Unbounded));
    vec![
        (PgTypeInfo::BOOL, vec![vec![1], b"t".to_vec()], read::<bool>),
        (PgTypeInfo::CHAR, vec![vec![7], b"a".to_vec()], read::<i8>),
        (PgTypeInfo::INT2, vec![encoded(&-2_i16)], read::<i16>),
        (PgTypeInfo::INT4, vec![encoded(&i32::MIN)], read::<u32>),
        (
            PgTypeInfo::INT8,
            vec![encoded(&-1_i64), b"42".to_vec()],
            read::<u64>,
        ),
        (PgTypeInfo::OID, vec![encoded(&Oid(25))], read::<Oid>),
        (PgTypeInfo::FLOAT4, vec![encoded(&1.5_f32)], read::<f32>),
        (
            PgTypeInfo::FLOAT8,
            vec![encoded(&f64::NAN), b"Infinity".to_vec()],
            read::<f64>,
        ),
        (PgTypeInfo::NUMERIC, numerics, read::<Decimal>),
        (
            PgTypeInfo::UUID,
            vec![encoded(&uuid::Uuid::from_u128(7))],
            read::<uuid::Uuid>,
        ),
        (PgTypeInfo::TEXT, text(&["héllo", ""]), read::<String>),
        (
            PgTypeInfo::BYTEA,
            vec![vec![0, 255], b"\\x00ff".to_vec()],
            read::<Vec<u8>>,
        ),
        (
            PgTypeInfo::JSON,
            text(&[r#"{"a":[1,2.5,null]}"#]),
            read::<Json<serde_json::Value>>,
        ),
        (
            PgTypeInfo::JSONB,
            vec![[&[1][..], br#"{"a":[1,true]}"#].concat()],
            read::<Json<serde_json::Value>>,
        ),
        (
            PgTypeInfo::DATE,
            vec![encoded(&at.date()), b"infinity".to_vec()],
            read::<NaiveDate>,
        ),
        (
            PgTypeInfo::TIME,
            vec![encoded(&at.time()), b"24:00:00".to_vec()],
            read::<NaiveTime>,
        ),
        (
            PgTypeInfo::TIMESTAMP,
            vec![encoded(&at)],
            read::<NaiveDateTime>,
        ),
        (
            PgTypeInfo::TIMESTAMPTZ,
            vec![
                encoded(&at.and_utc()),
                b"2020-02-29 23:59:01+05:30".to_vec(),
            ],
            read::<DateTime<FixedOffset>>,
        ),
        (
            PgTypeInfo::TIMETZ,
            vec![b"10:11:12.5+03".to_vec()],
            read::<PgTimeTz<NaiveTime, FixedOffset>>,
        ),
        (
            PgTypeInfo::INTERVAL,
            vec![encoded(&PgInterval {
                months: -14,
                days: 3,
                microseconds: 1,
            })],
            read::<PgInterval>,
        ),
        (
            PgTypeInfo::MONEY,
            vec![encoded(&PgMoney(-12_345))],
            read::<PgMoney>,
        ),
        (
            PgTypeInfo::INT4_ARRAY,
            vec![encoded(&vec![1_i32, 2, 3]), b"{{1,2},{3,4}}".to_vec()],
            read::<Vec<i32>>,
        ),
        (
            PgTypeInfo::INT4_ARRAY,
            vec![encoded(&vec![Some(1_i32), None, Some(3)])],
            read::<[Option<i32>; 3]>,
        ),
        (
            PgTypeInfo::TEXT_ARRAY,
            vec![encoded(&vec!["a,b", "\"q\""]), b"{\"a,b\",NULL,c}".to_vec()],
            read::<Vec<Option<String>>>,
        ),
        (
            PgTypeInfo::NUMERIC_ARRAY,
            vec![encoded(&decimals.to_vec())],
            read::<Vec<Decimal>>,
        ),
        (
            PgTypeInfo::TIMESTAMPTZ_ARRAY,
            vec![encoded(&vec![at.and_utc()])],
            read::<Vec<DateTime<Utc>>>,
        ),
        (
            PgTypeInfo::INT4_RANGE,
            vec![encoded(&int_range), b"[1,9)".to_vec(), b"empty".to_vec()],
            read::<PgRange<i32>>,
        ),
        (
            PgTypeInfo::NUM_RANGE,
            vec![encoded(&PgRange::<Decimal>::from((
                Bound::Unbounded,
                Bound::Included(decimals[0]),
            )))],
            read::<PgRange<Decimal>>,
        ),
        (
            PgTypeInfo::TSTZ_RANGE,
            vec![encoded(&PgRange::<DateTime<Utc>>::from((
                Bound::Included(at.and_utc()),
                Bound::Unbounded,
            )))],
            read::<PgRange<DateTime<Utc>>>,
        ),
        (
            PgTypeInfo::DATE_RANGE_ARRAY,
            vec![encoded(&vec![date_range])],
            read::<Vec<PgRange<NaiveDate>>>,
        ),
        (
            PgTypeInfo::POINT,
            vec![floats(&[1.0, -2.5]), b"(1,2)".to_vec()],
            read::<PgPoint>,
        ),
        (
            PgTypeInfo::LINE,
            vec![floats(&[1.0, 2.0, 3.0])],
            read::<PgLine>,
        ),
        (
            PgTypeInfo::LSEG,
            vec![floats(&[0.0, 0.0, 1.0, 1.0])],
            read::<PgLSeg>,
        ),
        (
            PgTypeInfo::BOX,
            vec![floats(&[2.0, 2.0, 0.0, 0.0])],
            read::<PgBox>,
        ),
        (
            PgTypeInfo::CIRCLE,
            vec![floats(&[0.0, 0.0, 5.0])],
            read::<PgCircle>,
        ),
        (
            PgTypeInfo::PATH,
            vec![
                [&[1, 0, 0, 0, 2][..], &floats(&[0.0, 0.0, 1.0, 1.0])].concat(),
                b"[(0,0),(1,1)]".to_vec(),
            ],
            read::<PgPath>,
        ),
        (
            PgTypeInfo::POLYGON,
            vec![[&[0, 0, 0, 2][..], &floats(&[0.0, 0.0, 1.0, 1.0])].concat()],
            read::<PgPolygon>,
        ),
        (
            named("cube"),
            vec![encoded(&PgCube::Point(1.5)), b"(1,2),(3,4)".to_vec()],
            read::<PgCube>,
        ),
        (
            named("hstore"),
            vec![encoded(&PgHstore(
                [("k".into(), Some("v".into())), ("n".into(), None)].into(),
            ))],
            read::<PgHstore>,
        ),
        (
            named("ltree"),
            vec![[&[1][..], b"a.b.c"].concat()],
            read::<PgLTree>,
        ),
        (
            named("lquery"),
            vec![[&[1][..], b"a.*{1,2}|b"].concat()],
            read::<PgLQuery>,
        ),
        (
            named("inet"),
            vec![[&[2, 24, 0, 4][..], &[10, 0, 0, 0]].concat()],
            read::<String>,
        ),
        (
            PgTypeInfo::RECORD,
            vec![record(), b"(7,x)".to_vec()],
            read::<(i32, String)>,
        ),
    ]
}

#[test]
fn every_type_decodes_or_fails_without_panicking() {
    for (seed, (ty, valid, read)) in (1..).zip(cases()) {
        fuzz(seed, 3000, &valid, |bytes| read(&ty, bytes));
    }
}

/// Values too short for their type, each of which panicked as it decoded.
#[test]
fn a_value_too_short_for_its_type_is_an_error() {
    use PgValueFormat::{Binary, Text};

    fn refused<T: for<'r> Decode<'r, Postgres>>(
        ty: PgTypeInfo,
        bytes: &[u8],
        format: PgValueFormat,
    ) {
        let value = PgValueRef {
            value: Some(bytes),
            row: None,
            type_info: ty.clone(),
            format,
        };
        assert!(T::decode(value).is_err(), "{ty} {bytes:?}");
    }

    refused::<bool>(PgTypeInfo::BOOL, b"", Binary);
    refused::<f32>(PgTypeInfo::FLOAT4, &[0; 3], Binary);
    refused::<f64>(PgTypeInfo::FLOAT8, &[0; 4], Binary);
    refused::<Oid>(PgTypeInfo::OID, &[0; 2], Binary);
    refused::<PgMoney>(PgTypeInfo::MONEY, &[0; 7], Binary);
    refused::<PgRange<i32>>(PgTypeInfo::INT4_RANGE, b"", Binary);
    // a range's text too short for its brackets, or with a bracket cut through a character
    for text in ["", "[", "é"] {
        refused::<PgRange<i32>>(PgTypeInfo::INT4_RANGE, text.as_bytes(), Text);
    }
    refused::<PgPoint>(PgTypeInfo::POINT, &[0; 15], Binary);
    refused::<PgLine>(PgTypeInfo::LINE, &[0; 16], Binary);
    refused::<PgLSeg>(PgTypeInfo::LSEG, &[0; 24], Binary);
    refused::<PgBox>(PgTypeInfo::BOX, &[0; 24], Binary);
    refused::<PgCircle>(PgTypeInfo::CIRCLE, &[0; 16], Binary);
}
