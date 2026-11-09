use super::*;

fn decode(kind: u32, data: &[u8]) -> Result<Authentication, Error> {
    let mut body = kind.to_be_bytes().to_vec();
    body.extend(data);
    Authentication::decode_body(body.into())
}

#[test]
fn truncated_messages_are_refused() {
    for body in [&b""[..], b"\0\0\0"] {
        assert!(Authentication::decode_body(Bytes::copy_from_slice(body)).is_err());
    }

    assert!(decode(5, b"abc").is_err());
    assert!(matches!(
        decode(5, b"abcd"),
        Ok(Authentication::Md5Password(AuthenticationMd5Password { salt })) if &salt == b"abcd"
    ));
}

#[test]
fn malformed_scram_attributes_are_refused() {
    for data in [&b""[..], b"r", b"ri4096", b"r=abc,,i=4096", b"r=abc,i"] {
        assert!(decode(11, data).is_err(), "{data:?}");
        assert!(decode(12, data).is_err(), "{data:?}");
    }
}

#[test]
fn the_iteration_count_is_required_and_positive() {
    for data in [
        &b"r=abc,s=c2FsdA=="[..],
        b"r=abc,s=c2FsdA==,i=",
        b"r=abc,s=c2FsdA==,i=0",
        b"r=abc,s=c2FsdA==,i=4096x",
    ] {
        assert!(decode(11, data).is_err(), "{data:?}");
    }

    let Ok(Authentication::SaslContinue(data)) = decode(11, b"r=abc,s=c2FsdA==,i=4096") else {
        panic!("a well-formed server-first-message");
    };
    assert_eq!(data.iterations, 4096);
    assert_eq!(data.nonce, "abc");
    assert_eq!(data.salt, b"salt");
}
