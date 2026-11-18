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

/// What [`scram_server`] saw: the client's first message without its GS2 header, and whether
/// its proof matched the password the server stores.
struct Seen {
    client_first_bare: String,
    proof_valid: bool,
}

/// A PostgreSQL server on a loopback port that authenticates one session with SCRAM-SHA-256
/// against `stored`, the password as the server keeps it, and signs its final message with the
/// right key only if `honest`.
fn scram_server(stored: &'static str, honest: bool) -> (u16, std::thread::JoinHandle<Seen>) {
    use crate::test_support::{read_body, send};
    use std::io::Read;

    fn message(stream: &mut std::net::TcpStream) -> Vec<u8> {
        let mut tag = [0];
        stream.read_exact(&mut tag).unwrap();
        assert_eq!(tag[0], b'p');
        read_body(stream).unwrap()
    }
    fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_body(&mut stream).unwrap(); // the startup message
        send(&mut stream, b'R', b"\0\0\0\x0aSCRAM-SHA-256\0\0").unwrap();

        // SASLInitialResponse: the mechanism, the response's length, then the response.
        let initial = message(&mut stream);
        let start = initial.iter().position(|&b| b == 0).unwrap() + 5;
        let first = String::from_utf8(initial[start..].to_vec()).unwrap();
        let client_first_bare = first.strip_prefix("n,,").unwrap().to_owned();
        // The nonce holds no `,` but may hold `r=`, so the first `,r=` starts it.
        let client_nonce = client_first_bare.split_once(",r=").unwrap().1.to_owned();

        let salt = b"salt and pepper";
        let server_first = format!(
            "r={client_nonce}server,s={},i=4096",
            BASE64_STANDARD.encode(salt)
        );
        let mut body = 11_u32.to_be_bytes().to_vec();
        body.extend(server_first.as_bytes());
        send(&mut stream, b'R', &body).unwrap();

        let last = String::from_utf8(message(&mut stream)).unwrap();
        let (without_proof, proof) = last.rsplit_once(",p=").unwrap();
        let salted = driver_core::rt::test_block_on(hi(stored, salt, 4096)).unwrap();
        let client_key = hmac(&salted, b"Client Key");
        let auth_message = format!("{client_first_bare},{server_first},{without_proof}");
        let signature = hmac(&Sha256::digest(&client_key), auth_message.as_bytes());
        let expected: Vec<u8> = client_key
            .iter()
            .zip(&signature)
            .map(|(a, b)| a ^ b)
            .collect();
        let proof_valid = BASE64_STANDARD.decode(proof).unwrap() == expected;

        let mut server_signature = hmac(&hmac(&salted, b"Server Key"), auth_message.as_bytes());
        if !honest {
            server_signature[0] ^= 1;
        }
        let mut body = 12_u32.to_be_bytes().to_vec();
        body.extend(format!("v={}", BASE64_STANDARD.encode(server_signature)).as_bytes());
        // From here on the client may have hung up already, refusing the signature.
        let _ = send(&mut stream, b'R', &body);
        if proof_valid {
            let _ = send(&mut stream, b'R', &0_u32.to_be_bytes());
            let _ = send(&mut stream, b'S', b"server_version\x0016.0\x00");
            let _ = send(&mut stream, b'Z', b"I");
        }
        let _ = stream.read_to_end(&mut Vec::new());
        Seen {
            client_first_bare,
            proof_valid,
        }
    });
    (port, server)
}

fn connect(port: u16, user: &str, password: &str) -> Result<(), Error> {
    use crate::{PgConnection, PgSslMode};
    use driver_core::connection::Connection;

    let options = PgConnectOptions::new_without_pgpass()
        .host("127.0.0.1")
        .port(port)
        .ssl_mode(PgSslMode::Disable)
        .username(user)
        .password(password);
    driver_core::rt::test_block_on(PgConnection::connect_with(&options)).map(drop)
}

#[test]
fn scram_leaves_the_user_name_to_the_startup_message() {
    // SCRAM would need a `,` in the name escaped; the server ignores this name anyway.
    let (port, server) = scram_server("pw1", true);
    connect(port, "audit,comma", "pw1").unwrap();
    let seen = server.join().unwrap();
    assert!(
        seen.client_first_bare.starts_with("n=,r="),
        "{}",
        seen.client_first_bare
    );
    assert!(seen.proof_valid);
}

#[test]
fn scram_sends_a_password_saslprep_refuses_as_it_is() {
    // Latin and Hebrew together break SASLprep's bidirectional rule; the server then keeps the
    // password unprepared, and so does libpq.
    let (port, server) = scram_server("abc שלום", true);
    connect(port, "user", "abc שלום").unwrap();
    assert!(server.join().unwrap().proof_valid);
}

#[test]
fn scram_prepares_a_password_saslprep_accepts() {
    // SASLprep maps a soft hyphen to nothing (RFC 4013's own example).
    let (port, server) = scram_server("IX", true);
    connect(port, "user", "I\u{AD}X").unwrap();
    assert!(server.join().unwrap().proof_valid);
}

#[test]
fn scram_refuses_a_server_that_cannot_prove_it_knows_the_password() {
    let (port, server) = scram_server("pw1", false);
    let error = connect(port, "user", "pw1").unwrap_err();
    assert!(matches!(error, Error::Protocol(_)), "{error:?}");
    server.join().unwrap();
}
