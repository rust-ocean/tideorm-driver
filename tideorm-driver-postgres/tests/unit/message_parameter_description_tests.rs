use super::*;

#[test]
fn test_decode_parameter_description() {
    const DATA: &[u8] = b"\x00\x02\x00\x00\x00\x00\x00\x00\x05\x00";

    let m = ParameterDescription::decode_body(DATA.into()).unwrap();

    assert_eq!(m.types.len(), 2);
    assert_eq!(m.types[0], Oid(0x0000_0000));
    assert_eq!(m.types[1], Oid(0x0000_0500));
}

#[test]
fn test_decode_empty_parameter_description() {
    const DATA: &[u8] = b"\x00\x00";

    let m = ParameterDescription::decode_body(DATA.into()).unwrap();

    assert!(m.types.is_empty());
}

#[test]
fn a_count_past_the_types_sent_is_refused() {
    for data in [&b""[..], b"\x00", b"\x00\x02\x00\x00\x00\x00"] {
        assert!(ParameterDescription::decode_body(Bytes::copy_from_slice(data)).is_err());
    }
}
