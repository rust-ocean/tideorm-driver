use super::*;

#[test]
fn test_decode_notification_response() {
    const NOTIFICATION_RESPONSE: &[u8] = b"\x34\x20\x10\x02TEST-CHANNEL\0THIS IS A TEST\0";

    let message = Notification::decode_body(Bytes::from(NOTIFICATION_RESPONSE)).unwrap();

    assert_eq!(message.process_id, 0x34201002);
    assert_eq!(&*message.channel, &b"TEST-CHANNEL"[..]);
    assert_eq!(&*message.payload, &b"THIS IS A TEST"[..]);
}

#[test]
fn a_notification_without_a_process_id_is_refused() {
    assert!(Notification::decode_body(Bytes::from_static(b"\x34\x20")).is_err());
}
