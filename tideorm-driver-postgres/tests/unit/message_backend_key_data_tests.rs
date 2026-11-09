use super::*;

#[test]
fn test_decode_backend_key_data() {
    const DATA: &[u8] = b"\0\0'\xc6\x89R\xc5+";

    let m = BackendKeyData::decode_body(DATA.into()).unwrap();

    assert_eq!(m.process_id, 10182);
    assert_eq!(m.secret_key, 2303903019);
}

#[test]
fn short_key_data_is_refused() {
    assert!(BackendKeyData::decode_body(Bytes::from_static(b"\0\0'\xc6")).is_err());
}
