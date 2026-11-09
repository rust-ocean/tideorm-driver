use super::*;

const HANDSHAKE_MYSQL_8_0_18: &[u8] = b"\n8.0.18\x00\x19\x00\x00\x00\x114aB0c\x06g\x00\xff\xff\xff\x02\x00\xff\xc7\x15\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00tL\x03s\x0f[4\rl4. \x00caching_sha2_password\x00";

#[test]
fn test_decode_handshake_mysql_8_0_18() {
    let p = Handshake::decode(HANDSHAKE_MYSQL_8_0_18.into()).unwrap();

    assert_eq!(p.protocol_version, 10);

    assert_eq!(
        p.server_capabilities,
        Capabilities::MYSQL
            | Capabilities::FOUND_ROWS
            | Capabilities::LONG_FLAG
            | Capabilities::CONNECT_WITH_DB
            | Capabilities::NO_SCHEMA
            | Capabilities::COMPRESS
            | Capabilities::ODBC
            | Capabilities::LOCAL_FILES
            | Capabilities::IGNORE_SPACE
            | Capabilities::PROTOCOL_41
            | Capabilities::INTERACTIVE
            | Capabilities::SSL
            | Capabilities::TRANSACTIONS
            | Capabilities::SECURE_CONNECTION
            | Capabilities::MULTI_STATEMENTS
            | Capabilities::MULTI_RESULTS
            | Capabilities::PS_MULTI_RESULTS
            | Capabilities::PLUGIN_AUTH
            | Capabilities::CONNECT_ATTRS
            | Capabilities::PLUGIN_AUTH_LENENC_DATA
            | Capabilities::CAN_HANDLE_EXPIRED_PASSWORDS
            | Capabilities::SESSION_TRACK
            | Capabilities::DEPRECATE_EOF
            | Capabilities::ZSTD_COMPRESSION_ALGORITHM
            | Capabilities::SSL_VERIFY_SERVER_CERT
            | Capabilities::OPTIONAL_RESULTSET_METADATA
            | Capabilities::REMEMBER_OPTIONS,
    );

    assert_eq!(p.server_default_collation, 255);
    assert!(p.status.contains(Status::SERVER_STATUS_AUTOCOMMIT));

    assert!(matches!(
        p.auth_plugin,
        Some(AuthPlugin::CachingSha2Password)
    ));

    assert_eq!(
        &*p.auth_plugin_data.into_iter().collect::<Vec<_>>(),
        &[17, 52, 97, 66, 48, 99, 6, 103, 116, 76, 3, 115, 15, 91, 52, 13, 108, 52, 46, 32,]
    );
}

#[test]
fn test_decode_handshake_mariadb_10_4_7() {
    const HANDSHAKE_MARIA_DB_10_4_7: &[u8] = b"\n5.5.5-10.4.7-MariaDB-1:10.4.7+maria~bionic\x00\x0b\x00\x00\x00t6L\\j\"dS\x00\xfe\xf7\x08\x02\x00\xff\x81\x15\x00\x00\x00\x00\x00\x00\x07\x00\x00\x00U14Oph9\"<H5n\x00mysql_native_password\x00";

    let p = Handshake::decode(HANDSHAKE_MARIA_DB_10_4_7.into()).unwrap();

    assert_eq!(p.protocol_version, 10);

    assert_eq!(
        &*p.server_version,
        "5.5.5-10.4.7-MariaDB-1:10.4.7+maria~bionic"
    );

    assert_eq!(
        p.server_capabilities,
        Capabilities::FOUND_ROWS
            | Capabilities::LONG_FLAG
            | Capabilities::CONNECT_WITH_DB
            | Capabilities::NO_SCHEMA
            | Capabilities::COMPRESS
            | Capabilities::ODBC
            | Capabilities::LOCAL_FILES
            | Capabilities::IGNORE_SPACE
            | Capabilities::PROTOCOL_41
            | Capabilities::INTERACTIVE
            | Capabilities::TRANSACTIONS
            | Capabilities::SECURE_CONNECTION
            | Capabilities::MULTI_STATEMENTS
            | Capabilities::MULTI_RESULTS
            | Capabilities::PS_MULTI_RESULTS
            | Capabilities::PLUGIN_AUTH
            | Capabilities::CONNECT_ATTRS
            | Capabilities::PLUGIN_AUTH_LENENC_DATA
            | Capabilities::CAN_HANDLE_EXPIRED_PASSWORDS
            | Capabilities::SESSION_TRACK
            | Capabilities::DEPRECATE_EOF
            | Capabilities::REMEMBER_OPTIONS
            | Capabilities::MARIADB_CLIENT_PROGRESS
            | Capabilities::MARIADB_CLIENT_MULTI
            | Capabilities::MARIADB_CLIENT_STMT_BULK_OPERATIONS
    );

    assert_eq!(p.server_default_collation, 8);
    assert!(p.status.contains(Status::SERVER_STATUS_AUTOCOMMIT));
    assert!(matches!(
        p.auth_plugin,
        Some(AuthPlugin::MySqlNativePassword)
    ));

    assert_eq!(
        &*p.auth_plugin_data.into_iter().collect::<Vec<_>>(),
        &[116, 54, 76, 92, 106, 34, 100, 83, 85, 49, 52, 79, 112, 104, 57, 34, 60, 72, 53, 110,]
    );
}

#[test]
fn a_truncated_handshake_is_refused() {
    for len in 0..HANDSHAKE_MYSQL_8_0_18.len() {
        let truncated = Bytes::copy_from_slice(&HANDSHAKE_MYSQL_8_0_18[..len]);
        assert!(Handshake::decode(truncated).is_err(), "{len}");
    }
}
