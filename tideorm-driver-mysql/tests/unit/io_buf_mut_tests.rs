use super::*;

#[test]
fn length_encoded_integers_take_the_shortest_form() {
    let cases: &[(u64, &[u8])] = &[
        (0xFA, b"\xFA"),
        // 0xFB and up do not fit one byte: 0xFB to 0xFF are markers
        (0xFB, b"\xFC\xFB\x00"),
        (0xFC, b"\xFC\xFC\x00"),
        (0xFD, b"\xFC\xFD\x00"),
        (0xFE, b"\xFC\xFE\x00"),
        (0xFF, b"\xFC\xFF\x00"),
        (u16::MAX as u64, b"\xFC\xFF\xFF"),
        (0xFF_FF_FF, b"\xFD\xFF\xFF\xFF"),
        (u64::MAX, b"\xFE\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF"),
    ];
    for &(value, bytes) in cases {
        let mut buf = Vec::new();
        buf.put_uint_lenenc(value);

        assert_eq!(buf, bytes, "{value:#X}");
    }
}

#[test]
fn test_encodes_byte_lenenc() {
    let mut buf = Vec::with_capacity(1024);
    buf.put_bytes_lenenc(b"random_string");

    assert_eq!(&buf[..], b"\x0Drandom_string");
}
