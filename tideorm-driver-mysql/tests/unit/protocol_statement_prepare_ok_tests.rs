use super::*;

#[test]
fn prepare_ok_reads_the_statement_and_its_counts() {
    let cases: [&[u8]; 2] = [
        b"\x00\x01\x02\x03\x04\x05\x00\x06\x00\x00\x07\x00",
        // MySQL 8's trailing metadata flag
        b"\x00\x01\x02\x03\x04\x05\x00\x06\x00\x00\x07\x00\x01",
    ];

    for bytes in cases {
        let ok = PrepareOk::decode_with(Bytes::from_static(bytes), Capabilities::empty()).unwrap();
        assert_eq!(
            (ok.statement_id, ok.columns, ok.params),
            (0x0403_0201, 5, 6),
            "{bytes:x?}"
        );
    }
}

#[test]
fn prepare_ok_refuses_short_or_foreign_packets() {
    let cases: [&[u8]; 3] = [
        b"",
        b"\x00\x01\x02\x03\x04\x05\x00\x06\x00\x00\x07",
        b"\xfe\x01\x02\x03\x04\x05\x00\x06\x00\x00\x07\x00",
    ];

    for bytes in cases {
        let error =
            PrepareOk::decode_with(Bytes::from_static(bytes), Capabilities::empty()).unwrap_err();
        assert!(matches!(error, Error::Protocol(_)), "{bytes:x?}: {error}");
    }
}
