use super::*;

#[test]
fn test_decode_err_packet_out_of_order() {
    const ERR_PACKETS_OUT_OF_ORDER: &[u8] = b"\xff\x84\x04Got packets out of order";

    let p =
        ErrPacket::decode_with(ERR_PACKETS_OUT_OF_ORDER.into(), Capabilities::PROTOCOL_41).unwrap();

    assert_eq!(&p.error_message, "Got packets out of order");
    assert_eq!(p.error_code, 1156);
    assert_eq!(p.sql_state, None);
}

#[test]
fn test_decode_err_packet_unknown_database() {
    const ERR_HANDSHAKE_UNKNOWN_DB: &[u8] = b"\xff\x19\x04#42000Unknown database \'unknown\'";

    let p =
        ErrPacket::decode_with(ERR_HANDSHAKE_UNKNOWN_DB.into(), Capabilities::PROTOCOL_41).unwrap();

    assert_eq!(p.error_code, 1049);
    assert_eq!(p.sql_state.as_deref(), Some("42000"));
    assert_eq!(&p.error_message, "Unknown database \'unknown\'");
}

#[test]
fn a_short_err_packet_is_refused() {
    for packet in [&b"\xff"[..], b"\xff\x84", b"\xff\x84\x04#4200"] {
        let packet = Bytes::copy_from_slice(packet);
        assert!(ErrPacket::decode_with(packet, Capabilities::PROTOCOL_41).is_err());
    }
}
