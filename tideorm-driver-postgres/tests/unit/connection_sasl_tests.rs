use super::*;
use crate::message::BackendMessage;

fn options() -> PgConnectOptions {
    PgConnectOptions::new_without_pgpass()
        .username("user")
        .password("secret")
}

/// An `AuthenticationSASL` offering `mechanisms`, each NUL-terminated.
fn offer(mechanisms: &[u8]) -> AuthenticationSasl {
    let mut body = 10_u32.to_be_bytes().to_vec();
    body.extend(mechanisms);
    body.push(0);
    match Authentication::decode_body(body.into()).unwrap() {
        Authentication::Sasl(data) => data,
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_offer_of_channel_binding_alone_is_refused() {
    driver_core::rt::test_block_on(async {
        let mut stream = PgStream::scripted(Vec::new());
        let error = authenticate(&mut stream, &options(), offer(b"SCRAM-SHA-256-PLUS\0"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("channel binding"), "{error}");
    });
}

#[test]
fn a_server_nonce_must_extend_the_client_nonce() {
    driver_core::rt::test_block_on(async {
        let data = b"r=not-the-client-nonce,s=c2FsdA==,i=4096";
        let mut input = vec![b'R'];
        input.extend((u32::try_from(data.len()).unwrap() + 8).to_be_bytes());
        input.extend(11_u32.to_be_bytes());
        input.extend(data);

        let mut stream = PgStream::scripted(input);
        let error = authenticate(&mut stream, &options(), offer(b"SCRAM-SHA-256\0"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("nonce"), "{error}");
    });
}
