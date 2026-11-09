use super::*;

#[test]
fn ok_packets_are_read_with_either_header() {
    // (packet, affected rows, last insert ID, warnings, status)
    #[rustfmt::skip]
    let cases: [(&[u8], u64, u64, u16, Status); 3] = [
        (
            b"\x00\x00\x00\x02@\x00\x00", 0, 0, 0,
            Status::SERVER_STATUS_AUTOCOMMIT | Status::SERVER_SESSION_STATE_CHANGED,
        ),
        // 0xfe header, with an info string after the fixed fields
        (b"\xfe\x01\x00\x02\x00\x00\x00\x05\x09info data", 1, 0, 0, Status::SERVER_STATUS_AUTOCOMMIT),
        (b"\xfe\x05\x64\x02\x00\x01\x00\x0e\x14extended information", 5, 100, 1, Status::SERVER_STATUS_AUTOCOMMIT),
    ];

    for (packet, affected_rows, last_insert_id, warnings, status) in cases {
        let ok = OkPacket::decode(Bytes::from_static(packet)).unwrap();
        assert_eq!(
            (ok.affected_rows, ok.last_insert_id, ok.warnings, ok.status),
            (affected_rows, last_insert_id, warnings, status),
            "{packet:?}"
        );
    }
}

#[test]
fn ok_packets_without_status_and_warnings_are_refused() {
    for packet in [&b"\x00\x00\x00\x01"[..], b"\x00\x00\x00"] {
        let error = OkPacket::decode(Bytes::from_static(packet)).unwrap_err();
        assert!(matches!(error, Error::Protocol(_)), "{packet:?}: {error}");
    }
}
