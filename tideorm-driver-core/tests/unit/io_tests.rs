use super::*;

#[test]
fn put_str_nul_appends_the_text_and_one_nul() {
    // (bytes already in the buffer, text, expected buffer)
    let cases: [(&[u8], &str, &[u8]); 3] = [
        (b"", "", b"\0"),
        (b"", "user", b"user\0"),
        (b"Q", "SELECT 1", b"QSELECT 1\0"),
    ];

    for (before, text, expected) in cases {
        let mut buf = before.to_vec();
        buf.put_str_nul(text);
        assert_eq!(buf, expected, "{text:?}");
    }
}

#[test]
fn raw_frames_pass_through_unchanged() {
    let frame = Bytes::from_static(b"\x01\x02\x03");

    assert_eq!(Bytes::decode(frame.clone()).unwrap(), frame);
    <()>::decode(frame.clone()).unwrap();

    // The blanket impl takes any context, so the context is named.
    let mut buf = b"head".to_vec();
    <&[u8] as ProtocolEncode<'_>>::encode(&&frame[..], &mut buf).unwrap();
    <&[u8] as ProtocolEncode<'_, u8>>::encode_with(&&frame[..], &mut buf, 7).unwrap();
    assert_eq!(buf, b"head\x01\x02\x03\x01\x02\x03");
}
