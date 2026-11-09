use super::*;
use crate::PgValueFormat;

#[test]
fn resolver_accepts_multirange_type_code_in_both_protocols() {
    for format in [PgValueFormat::Text, PgValueFormat::Binary] {
        let value = PgValueRef {
            value: Some(b"m"),
            row: None,
            type_info: PgTypeInfo(PgType::Char),
            format,
        };
        assert_eq!(TypType::decode(value).unwrap(), TypType::Multirange);
    }
    assert!(TypType::try_from(i8::try_from(b'?').unwrap()).is_err());
}
