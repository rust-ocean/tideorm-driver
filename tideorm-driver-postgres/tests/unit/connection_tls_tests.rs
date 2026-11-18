use super::*;
use crate::net::connect_tcp;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// Read only when the connection verifies, so any bytes stand in for a certificate here.
const ROOT: &[u8] = b"a root certificate";

fn mode(mode: PgSslMode) -> PgConnectOptions {
    PgConnectOptions::new_without_pgpass().ssl_mode(mode)
}

#[test]
fn require_checks_the_chain_when_given_a_root_certificate() {
    assert!(!verifies_certificate(&mode(PgSslMode::Require)));
    assert!(verifies_certificate(
        &mode(PgSslMode::Require).ssl_root_cert_from_pem(ROOT.to_vec())
    ));
    assert!(verifies_certificate(&mode(PgSslMode::VerifyCa)));
    assert!(verifies_certificate(&mode(PgSslMode::VerifyFull)));
    assert!(!verifies_certificate(
        &mode(PgSslMode::Prefer).ssl_root_cert_from_pem(ROOT.to_vec())
    ));
}

/// Upgrades a connection to a server that reads the `SSLRequest`, answers `answer` and closes.
fn upgrade_against(answer: u8, options: PgConnectOptions) -> crate::Result<Box<dyn Socket>> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 8];
        stream.read_exact(&mut request).unwrap();
        assert_eq!(request, SslRequest::BYTES);
        stream.write_all(&[answer]).unwrap();
    });
    let result =
        driver_core::rt::test_block_on(connect_tcp("127.0.0.1", port, MaybeUpgradeTls(&options)))
            .and_then(|upgraded| upgraded);
    server.join().unwrap();
    result
}

#[test]
fn only_prefer_goes_on_unencrypted_when_the_server_refuses_tls() {
    assert!(upgrade_against(b'N', mode(PgSslMode::Prefer)).is_ok());
    for refused in [
        PgSslMode::Require,
        PgSslMode::VerifyCa,
        PgSslMode::VerifyFull,
    ] {
        match upgrade_against(b'N', mode(refused)) {
            Err(Error::Tls(_)) => {}
            other => panic!("{refused:?}: {:?}", other.map(drop)),
        }
    }
}

#[test]
fn a_failed_handshake_never_falls_back_to_plain_text() {
    // The server agrees to TLS and closes: even `prefer` must not go on unencrypted.
    if crate::net::tls::available() {
        assert!(upgrade_against(b'S', mode(PgSslMode::Prefer)).is_err());
    }
    assert!(upgrade_against(b'S', mode(PgSslMode::Require)).is_err());
}
