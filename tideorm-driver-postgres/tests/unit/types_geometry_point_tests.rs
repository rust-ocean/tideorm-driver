use std::str::FromStr;

use crate::test_support::encoded;

use super::PgPoint;

const POINT_BYTES: &[u8] = &[
    64, 0, 204, 204, 204, 204, 204, 205, 64, 20, 204, 204, 204, 204, 204, 205,
];

#[test]
fn can_deserialise_point_type_bytes() {
    let point = PgPoint::from_bytes(POINT_BYTES).unwrap();
    assert_eq!(point, PgPoint { x: 2.1, y: 5.2 })
}

#[test]
fn can_deserialise_point_type_str() {
    let point = PgPoint::from_str("(2, 3)").unwrap();
    assert_eq!(point, PgPoint { x: 2., y: 3. });
}

#[test]
fn can_deserialise_point_type_str_float() {
    let point = PgPoint::from_str("(2.5, 3.4)").unwrap();
    assert_eq!(point, PgPoint { x: 2.5, y: 3.4 });
}

#[test]
fn can_serialise_point_type() {
    let point = PgPoint { x: 2.1, y: 5.2 };
    assert_eq!(encoded(&point), POINT_BYTES,)
}
