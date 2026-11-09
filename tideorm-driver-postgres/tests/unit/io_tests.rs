use super::*;

#[test]
fn length_prefix_counts_itself_and_the_body() {
    // (name, bytes already in the buffer, body)
    let cases: [(&str, &[u8], &[u8]); 3] = [
        ("empty body", b"", b""),
        ("query", b"", b"SELECT 1\0"),
        ("after another message", b"S\0\0\0\x04", b"abc"),
    ];

    for (name, before, body) in cases {
        let mut buf = before.to_vec();
        buf.put_length_prefixed(|buf| {
            buf.extend_from_slice(body);
            Ok(())
        })
        .unwrap();

        let length = i32::try_from(4 + body.len()).unwrap();
        let expected = [before, &length.to_be_bytes(), body].concat();
        assert_eq!(buf, expected, "{name}");
    }
}

#[test]
fn a_failed_body_leaves_the_buffer_as_it_was() {
    let mut buf = b"kept".to_vec();
    let error = buf
        .put_length_prefixed(|buf| {
            buf.extend_from_slice(b"partial");
            Err(err_protocol!("body refused"))
        })
        .unwrap_err();

    assert_eq!(buf, b"kept");
    assert!(error.to_string().contains("body refused"), "{error}");
}

#[test]
fn names_end_with_a_nul_and_match_their_announced_length() {
    let statements: [(StatementId, &[u8]); 4] = [
        (StatementId::UNNAMED, b"\0"),
        (StatementId::NAMED_START, b"tide_s_1\0"),
        (StatementId::NAMED_START.next(), b"tide_s_2\0"),
        (StatementId::TEST_VAL, b"tide_s_1234567890\0"),
    ];

    for (id, expected) in statements {
        let mut buf = Vec::new();
        buf.put_statement_name(id);
        assert_eq!(buf, expected, "{id:?}");
        assert_eq!(id.name_len().0, buf.len(), "{id:?}");
    }

    let mut buf = Vec::new();
    buf.put_portal_name(PortalId::UNNAMED);
    assert_eq!(buf, b"\0");
    assert_eq!(PortalId::UNNAMED.name_len().0, buf.len());
}
