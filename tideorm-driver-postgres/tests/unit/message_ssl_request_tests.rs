use super::*;

/// The request is its length (8, counting itself) and then the code 80877103: 1234 in the high
/// 16 bits and 5679 in the low ones, a value no protocol version will take.
/// <https://www.postgresql.org/docs/current/protocol-message-formats.html#PROTOCOL-MESSAGE-FORMATS-SSLREQUEST>
#[test]
fn ssl_request_is_its_length_and_code() {
    let code: u32 = (1234 << 16) | 5679;
    assert_eq!(code, 80_877_103);
    let expected = [8_u32.to_be_bytes(), code.to_be_bytes()].concat();

    let mut encoded = Vec::new();
    SslRequest.encode(&mut encoded).unwrap();

    assert_eq!(SslRequest::BYTES, expected);
    assert_eq!(encoded, expected);
}
