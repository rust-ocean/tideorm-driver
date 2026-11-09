use super::*;

#[test]
fn parameter_status_reads_name_and_value() {
    // the body is two NUL-terminated strings
    let cases: [(&[u8], &str, &str); 2] = [
        (b"client_encoding\0UTF8\0", "client_encoding", "UTF8"),
        (b"\0\0", "", ""),
    ];

    for (body, name, value) in cases {
        let status = ParameterStatus::decode_body(body.into()).unwrap();
        assert_eq!((status.name.as_str(), status.value.as_str()), (name, value));
    }

    // a string missing its NUL is refused
    let unterminated = Bytes::from_static(b"client_encoding\0UTF8");
    assert!(ParameterStatus::decode_body(unterminated).is_err());
}
