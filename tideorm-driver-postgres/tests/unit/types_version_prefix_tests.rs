//! Binary `jsonb`, `ltree` and `lquery` values start with a format version byte.

use std::str::FromStr;

use driver_core::types::Json;
use serde_json::Value as JsonValue;

use super::{PgLQuery, PgLQueryLevel, PgLTree};
use crate::decode::Decode;
use crate::test_support::binary;
use crate::types::Type;
use crate::{PgTypeInfo, Postgres};

fn decode<'r, T: Decode<'r, Postgres>>(
    bytes: &'r [u8],
    type_info: PgTypeInfo,
) -> Result<T, String> {
    T::decode(binary(bytes, type_info)).map_err(|error| error.to_string())
}

#[test]
fn an_empty_binary_value_is_an_error_not_a_panic() {
    assert!(decode::<Json<JsonValue>>(b"", PgTypeInfo::JSONB).is_err());
    assert!(decode::<PgLTree>(b"", <PgLTree as Type<Postgres>>::type_info()).is_err());
    assert!(decode::<PgLQuery>(b"", <PgLQuery as Type<Postgres>>::type_info()).is_err());
}

#[test]
fn the_version_byte_is_checked_then_skipped() {
    let Json(document) = decode::<Json<JsonValue>>(b"\x01{\"a\":1}", PgTypeInfo::JSONB).unwrap();
    assert_eq!(document, serde_json::json!({ "a": 1 }));
    assert!(decode::<Json<JsonValue>>(b"\x02{}", PgTypeInfo::JSONB).is_err());

    let ltree = <PgLTree as Type<Postgres>>::type_info();
    assert_eq!(
        decode::<PgLTree>(b"\x01a.b", ltree.clone())
            .unwrap()
            .to_string(),
        "a.b"
    );
    assert!(decode::<PgLTree>(b"\x02a.b", ltree).is_err());

    let lquery = <PgLQuery as Type<Postgres>>::type_info();
    assert_eq!(
        decode::<PgLQuery>(b"\x01a.*{1,2}", lquery.clone())
            .unwrap()
            .to_string(),
        "a.*{1,2}"
    );
    assert!(decode::<PgLQuery>(b"\x02a", lquery).is_err());
}

#[test]
fn a_star_level_takes_only_braced_bounds() {
    assert!(matches!(
        PgLQueryLevel::from_str("*{2}"),
        Ok(PgLQueryLevel::Star(Some(2), Some(2)))
    ));
    assert!(matches!(
        PgLQueryLevel::from_str("*"),
        Ok(PgLQueryLevel::Star(None, None))
    ));
    // PostgreSQL writes a level with one bound left out this way, and reads it back
    for open in ["*{2,}", "*{,3}"] {
        assert_eq!(PgLQueryLevel::from_str(open).unwrap().to_string(), open);
    }
    for malformed in ["*a", "*{", "*}", "*é", "*{1", "*{}", "*{1,2,3}"] {
        assert!(PgLQueryLevel::from_str(malformed).is_err(), "{malformed}");
    }
}
