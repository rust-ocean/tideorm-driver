use super::*;

#[test]
fn test_encode_parse() {
    const EXPECTED: &[u8] = b"P\0\0\0\x26tide_s_1234567890\0SELECT $1\0\0\x01\0\0\0\x19";

    let mut buf = Vec::new();
    let m = Parse {
        statement: StatementId::TEST_VAL,
        query: "SELECT $1",
        param_types: &[Oid(25)],
    };

    m.encode_msg(&mut buf).unwrap();

    assert_eq!(buf, EXPECTED);
}

#[test]
fn test_encode_parse_unnamed_statement() {
    const EXPECTED: &[u8] = b"P\0\0\0\x15\0SELECT $1\0\0\x01\0\0\0\x19";

    let mut buf = Vec::new();
    let m = Parse {
        statement: StatementId::UNNAMED,
        query: "SELECT $1",
        param_types: &[Oid(25)],
    };

    m.encode_msg(&mut buf).unwrap();

    assert_eq!(buf, EXPECTED);
}
