use super::*;
use crate::test_support::text;

fn fields(literal: &str, count: usize) -> Result<Vec<Option<String>>, BoxDynError> {
    let mut decoder = PgRecordDecoder::new(text(literal, PgTypeInfo::RECORD))?;
    (0..count).map(|_| decoder.try_decode()).collect()
}

fn binary(bytes: &[u8]) -> PgValueRef<'_> {
    PgValueRef {
        value: Some(bytes),
        row: None,
        type_info: PgTypeInfo::RECORD,
        format: PgValueFormat::Binary,
    }
}

#[test]
fn text_records_keep_utf8_and_a_trailing_null() {
    assert_eq!(
        fields(r#"(café,"naïve, quoted")"#, 2).unwrap(),
        [Some("café".to_string()), Some("naïve, quoted".to_string())]
    );
    assert_eq!(fields("(1,)", 2).unwrap(), [Some("1".to_string()), None]);
    assert_eq!(fields("(,)", 2).unwrap(), [None, None]);
    assert!(fields("(1)", 2).is_err());
}

#[test]
fn malformed_records_fail_without_panicking() {
    for literal in ["", "(", "1,2"] {
        assert!(
            PgRecordDecoder::new(text(literal, PgTypeInfo::RECORD)).is_err(),
            "{literal:?}"
        );
    }
    assert!(PgRecordDecoder::new(binary(&[0, 0])).is_err());
    // One field announced, its type oid cut short.
    let mut decoder = PgRecordDecoder::new(binary(&[0, 0, 0, 1, 0, 0])).unwrap();
    assert!(decoder.try_decode::<i32>().is_err());
}
