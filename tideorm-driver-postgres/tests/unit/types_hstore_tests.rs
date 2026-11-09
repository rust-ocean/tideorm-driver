use super::*;
use crate::PgValueFormat;

/// Binary `hstore` values and the maps they hold. The format is the pair count, then each key
/// and value as a length (`-1` for a NULL value) and its bytes, every number a big-endian
/// `i32`. The pairs are in key order, the order a `BTreeMap` encodes them in.
const CASES: &[(&str, Pairs)] = &[
    ("00000000", &[]),
    (
        "00000003\
         00000003616765 ffffffff\
         000000046e616d65 000000044a6f686e\
         000000077375726e616d65 00000003446f65",
        &[
            ("age", None),
            ("name", Some("John")),
            ("surname", Some("Doe")),
        ],
    ),
];

type Pairs = &'static [(&'static str, Option<&'static str>)];

fn hstore(pairs: Pairs) -> PgHstore {
    pairs
        .iter()
        .map(|&(key, value)| (key.to_owned(), value.map(str::to_owned)))
        .collect()
}

fn binary(bytes: &[u8]) -> PgValueRef<'_> {
    PgValueRef {
        value: Some(bytes),
        row: None,
        type_info: PgTypeInfo::with_name("hstore"),
        format: PgValueFormat::Binary,
    }
}

fn bytes(hex: &str) -> Vec<u8> {
    hex::decode(hex.replace(' ', "")).unwrap()
}

#[test]
fn hstore_decodes_binary() {
    for &(hex, pairs) in CASES {
        let bytes = bytes(hex);
        assert_eq!(
            PgHstore::decode(binary(&bytes)).unwrap(),
            hstore(pairs),
            "{hex}"
        );
    }
}

#[test]
fn hstore_encodes_binary() {
    for &(hex, pairs) in CASES {
        let mut buf = PgArgumentBuffer::default();
        assert!(matches!(
            hstore(pairs).encode_by_ref(&mut buf),
            Ok(IsNull::No)
        ));
        assert_eq!(*buf, bytes(hex), "{pairs:?}");
    }
}

#[test]
fn hstore_refuses_malformed_binary() {
    let cases = [
        ("fffffffb", "PgHstore: length out of range: -5"),
        (
            "00000001",
            "PgHstore: error reading 0th key: expected 4 bytes, got 0",
        ),
        (
            "00000001 ffffffff",
            "PgHstore: expected 0th key, got nothing",
        ),
    ];

    for (hex, error) in cases {
        let bytes = bytes(hex);
        let decoded = PgHstore::decode(binary(&bytes));
        assert_eq!(decoded.unwrap_err().to_string(), error, "{hex}");
    }
}
