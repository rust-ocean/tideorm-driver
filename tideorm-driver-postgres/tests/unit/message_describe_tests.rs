use crate::message::FrontendMessage;

use super::{Describe, StatementId};

#[test]
fn test_encode_describe_statement() {
    const EXPECTED: &[u8] = b"D\0\0\0\x17Stide_s_1234567890\0";

    let mut buf = Vec::new();
    let m = Describe::Statement(StatementId::TEST_VAL);

    m.encode_msg(&mut buf).unwrap();

    assert_eq!(buf, EXPECTED);
}

#[test]
fn test_encode_describe_unnamed_statement() {
    const EXPECTED: &[u8] = b"D\0\0\0\x06S\0";

    let mut buf = Vec::new();
    let m = Describe::Statement(StatementId::UNNAMED);

    m.encode_msg(&mut buf).unwrap();

    assert_eq!(buf, EXPECTED);
}
