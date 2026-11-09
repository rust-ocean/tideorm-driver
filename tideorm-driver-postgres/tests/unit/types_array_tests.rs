use super::*;
use crate::test_support::text;
use crate::types::PgBox;
use std::str::FromStr;

#[test]
fn text_arrays_split_on_commas() {
    let values =
        <Vec<Option<i32>> as Decode<Postgres>>::decode(text("{1,NULL,3}", PgTypeInfo::INT4_ARRAY))
            .unwrap();
    assert_eq!(values, [Some(1), None, Some(3)]);
}

#[test]
fn text_box_arrays_split_on_semicolons() {
    let boxes = <Vec<PgBox> as Decode<Postgres>>::decode(text(
        "{(3,4),(1,2);(0,0),(-1,-1)}",
        PgTypeInfo::BOX_ARRAY,
    ))
    .unwrap();
    assert_eq!(
        boxes,
        [
            PgBox::from_str("(3,4),(1,2)").unwrap(),
            PgBox::from_str("(0,0),(-1,-1)").unwrap(),
        ]
    );
}

#[test]
fn a_quoted_or_escaped_null_is_text() {
    let values = <Vec<Option<String>> as Decode<Postgres>>::decode(text(
        r#"{"NULL",NULL,\NULL}"#,
        PgTypeInfo::TEXT_ARRAY,
    ))
    .unwrap();
    assert_eq!(
        values,
        [Some("NULL".to_string()), None, Some("NULL".to_string())]
    );
}

#[test]
fn malformed_arrays_fail_without_panicking() {
    for literal in ["", "{", "[0:1]={1,2}"] {
        let value = text(literal, PgTypeInfo::INT4_ARRAY);
        assert!(
            <Vec<i32> as Decode<Postgres>>::decode(value).is_err(),
            "{literal:?}"
        );
    }
    // A header cut short, and an element longer than the value.
    let short: &[u8] = &[0, 0, 0];
    let long: &[u8] = &[
        0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 23, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 9, 1,
    ];
    for bytes in [short, long] {
        let value = PgValueRef {
            value: Some(bytes),
            row: None,
            type_info: PgTypeInfo::INT4_ARRAY,
            format: PgValueFormat::Binary,
        };
        assert!(<Vec<i32> as Decode<Postgres>>::decode(value).is_err());
    }
}
