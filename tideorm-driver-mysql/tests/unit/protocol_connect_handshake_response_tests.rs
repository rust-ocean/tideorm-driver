use crate::error::Error;
use crate::io::ProtocolEncode;
use crate::protocol::auth::AuthPlugin;
use crate::protocol::connect::{HandshakeResponse, SslRequest};
use crate::protocol::Capabilities as C;

fn encode<'en>(packet: &impl ProtocolEncode<'en, C>, capabilities: C) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();
    packet.encode_with(&mut buf, capabilities)?;
    Ok(buf)
}

/// The 32 bytes both packets start with, for a 1024-byte packet limit and
/// `utf8mb4_general_ci`.
fn prefix(capabilities: C, last: [u8; 4]) -> Vec<u8> {
    let low = u32::try_from(capabilities.bits() & 0xFFFF_FFFF).unwrap();
    [
        &low.to_le_bytes()[..],
        &1024_u32.to_le_bytes(),
        &[45],
        &[0; 19],
        &last,
    ]
    .concat()
}

fn response<'a>(
    database: Option<&'a str>,
    auth_plugin: Option<AuthPlugin>,
    auth_response: Option<&'a [u8]>,
) -> HandshakeResponse<'a> {
    HandshakeResponse {
        database,
        max_packet_size: 1024,
        charset: 45,
        username: "root",
        auth_plugin,
        auth_response,
    }
}

#[test]
fn ssl_request_ends_with_the_extended_capabilities_unless_the_server_is_mysql() {
    let request = SslRequest {
        max_packet_size: 1024,
        charset: 45,
    };
    let mariadb = C::PROTOCOL_41 | C::SSL | C::MARIADB_CLIENT_EXTENDED_TYPE_INFO;
    let mysql = mariadb | C::MYSQL;

    for (capabilities, last) in [(mariadb, [8, 0, 0, 0]), (mysql, [0; 4])] {
        assert_eq!(
            encode(&request, capabilities).unwrap(),
            prefix(capabilities, last),
            "{capabilities:?}"
        );
    }
}

#[test]
fn handshake_response_follows_the_negotiated_capabilities() {
    let base = C::PROTOCOL_41;
    let secure = base | C::SECURE_CONNECTION;
    let full = secure | C::PLUGIN_AUTH | C::PLUGIN_AUTH_LENENC_DATA | C::CONNECT_WITH_DB;

    #[rustfmt::skip]
    let cases: [(&str, C, HandshakeResponse, Vec<u8>); 5] = [
        (
            "length-encoded auth data, database and plugin",
            full, response(Some("db"), Some(AuthPlugin::CachingSha2Password), Some(&[0xAA, 0xBB])),
            [prefix(full, [0; 4]), b"root\0\x02\xAA\xBBdb\0caching_sha2_password\0".to_vec()].concat(),
        ),
        (
            "no plugin: PLUGIN_AUTH is left out, and so is a database the capabilities do not name",
            secure | C::PLUGIN_AUTH, response(Some("db"), None, Some(&[1, 2, 3])),
            [prefix(secure, [0; 4]), b"root\0\x03\x01\x02\x03".to_vec()].concat(),
        ),
        (
            "one length byte with no auth data",
            secure, response(None, None, None),
            [prefix(secure, [0; 4]), b"root\0\0".to_vec()].concat(),
        ),
        (
            "without SECURE_CONNECTION the auth data is one zero byte",
            base, response(None, None, Some(&[1, 2])),
            [prefix(base, [0; 4]), b"root\0\0".to_vec()].concat(),
        ),
        (
            "CONNECT_WITH_DB with no database writes an empty name",
            base | C::CONNECT_WITH_DB | C::PLUGIN_AUTH, response(None, Some(AuthPlugin::MySqlNativePassword), None),
            [
                prefix(base | C::CONNECT_WITH_DB | C::PLUGIN_AUTH, [0; 4]),
                b"root\0\0\0mysql_native_password\0".to_vec(),
            ]
            .concat(),
        ),
    ];

    for (name, capabilities, response, expected) in cases {
        assert_eq!(encode(&response, capabilities).unwrap(), expected, "{name}");
    }
}

#[test]
fn handshake_response_refuses_auth_data_past_255_bytes_without_lenenc() {
    let long = [0_u8; 256];
    let error = encode(
        &response(None, None, Some(&long)),
        C::PROTOCOL_41 | C::SECURE_CONNECTION,
    )
    .unwrap_err();
    assert!(matches!(error, Error::Protocol(_)), "{error}");
}
