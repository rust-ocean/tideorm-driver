//! The certificate checks of the rustls handshake, against a rustls server on a loopback port.

use super::*;
use crate::net::tls::CertificateInput;
use crate::net::{connect_tcp, SocketIntoBox};
use std::net::TcpListener;
use std::thread;

const CA: &str = include_str!("../fixtures/tls/ca.pem");
const OTHER_CA: &str = include_str!("../fixtures/tls/other-ca.pem");
const SERVER: (&str, &str) = (
    include_str!("../fixtures/tls/server.pem"),
    include_str!("../fixtures/tls/server.key"),
);
const EXPIRED: (&str, &str) = (
    include_str!("../fixtures/tls/expired.pem"),
    include_str!("../fixtures/tls/expired.key"),
);

/// How far the client trusts the server, as the drivers' SSL modes map it.
#[derive(Clone, Copy)]
enum Check {
    Nothing,
    Authority,
    AuthorityAndHost,
}

/// Runs one handshake with a server presenting `(cert, key)`, the client trusting `root` and
/// naming `host`.
fn handshake_with(
    (cert, key): (&'static str, &'static str),
    check: Check,
    root: Option<&'static str>,
    host: &str,
) -> Result<(), Error> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        certs_from_pem(cert.as_bytes().to_vec()).unwrap(),
        private_key_from_pem(key.as_bytes().to_vec()).unwrap(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
        while connection.is_handshaking() {
            if connection.complete_io(&mut stream).is_err() {
                break;
            }
        }
    });

    let root = root.map(|pem| CertificateInput::Inline(pem.as_bytes().to_vec()));
    let result = crate::rt::test_block_on(async {
        let socket = connect_tcp("127.0.0.1", port, SocketIntoBox).await?;
        handshake(
            socket,
            TlsConfig {
                accept_invalid_certs: matches!(check, Check::Nothing),
                accept_invalid_hostnames: !matches!(check, Check::AuthorityAndHost),
                hostname: host,
                root_cert_path: root.as_ref(),
                client_cert_path: None,
                client_key_path: None,
            },
        )
        .await
        .map(drop)
    });
    server.join().unwrap();
    result
}

#[track_caller]
fn assert_refused(result: Result<(), Error>, expected: &str) {
    match result {
        Err(Error::Tls(error)) => assert!(error.to_string().contains(expected), "{error}"),
        other => panic!("expected a TLS error naming {expected:?}, got {other:?}"),
    }
}

#[test]
fn a_certificate_from_the_given_authority_is_trusted() {
    handshake_with(SERVER, Check::Authority, Some(CA), "localhost").unwrap();
    handshake_with(SERVER, Check::AuthorityAndHost, Some(CA), "localhost").unwrap();
}

#[test]
fn a_certificate_from_another_authority_is_refused() {
    assert_refused(
        handshake_with(SERVER, Check::Authority, Some(OTHER_CA), "localhost"),
        "UnknownIssuer",
    );
    assert_refused(
        handshake_with(SERVER, Check::AuthorityAndHost, Some(OTHER_CA), "localhost"),
        "UnknownIssuer",
    );
}

#[test]
fn an_expired_certificate_is_refused() {
    assert_refused(
        handshake_with(EXPIRED, Check::Authority, Some(CA), "localhost"),
        "expired",
    );
}

#[test]
fn only_the_host_check_refuses_another_host_name() {
    handshake_with(SERVER, Check::Authority, Some(CA), "db.example.com").unwrap();
    assert_refused(
        handshake_with(SERVER, Check::AuthorityAndHost, Some(CA), "db.example.com"),
        "not valid for name",
    );
}

#[test]
fn an_unchecked_connection_takes_any_certificate() {
    handshake_with(EXPIRED, Check::Nothing, None, "db.example.com").unwrap();
}

#[test]
fn checking_the_authority_alone_needs_a_named_root() {
    // Any public authority signs certificates for any domain, so with the web roots this check
    // would accept every publicly issued certificate.
    assert_refused(
        handshake_with(SERVER, Check::Authority, None, "localhost"),
        "needs the root certificate",
    );
}

#[test]
fn a_named_root_replaces_the_web_roots() {
    let root = CertificateInput::Inline(CA.as_bytes().to_vec());
    let config = TlsConfig {
        accept_invalid_certs: false,
        accept_invalid_hostnames: false,
        hostname: "localhost",
        root_cert_path: Some(&root),
        client_cert_path: None,
        client_key_path: None,
    };
    let store = crate::rt::test_block_on(trusted_roots(&config)).unwrap();
    assert_eq!(store.len(), 1);

    let no_root = TlsConfig {
        root_cert_path: None,
        ..config
    };
    let store = crate::rt::test_block_on(trusted_roots(&no_root)).unwrap();
    assert_eq!(store.len(), webpki_roots::TLS_SERVER_ROOTS.len());
}

#[test]
fn a_root_file_without_a_certificate_is_refused() {
    let empty = CertificateInput::Inline(b"no certificate here".to_vec());
    let config = TlsConfig {
        accept_invalid_certs: false,
        accept_invalid_hostnames: false,
        hostname: "localhost",
        root_cert_path: Some(&empty),
        client_cert_path: None,
        client_key_path: None,
    };
    match crate::rt::test_block_on(trusted_roots(&config)) {
        Err(Error::Tls(error)) => assert!(error.to_string().contains("no certificate"), "{error}"),
        other => panic!("{:?}", other.map(|store| store.len())),
    }
}
