use super::*;

#[test]
fn a_short_eof_packet_is_refused() {
    for len in 0..5 {
        let packet = Bytes::copy_from_slice(&[0xfe, 0, 0, 2, 0][..len]);
        assert!(
            EofPacket::decode_with(packet, Capabilities::empty()).is_err(),
            "{len}"
        );
    }

    let eof = EofPacket::decode_with(
        Bytes::from_static(&[0xfe, 0, 0, 2, 0]),
        Capabilities::empty(),
    );
    assert!(eof
        .unwrap()
        .status
        .contains(Status::SERVER_STATUS_AUTOCOMMIT));
}
