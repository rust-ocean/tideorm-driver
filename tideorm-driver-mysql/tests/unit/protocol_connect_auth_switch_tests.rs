use super::*;

const NONCE: &[u8] = b"abcdefghijabcdefghij";

/// The plugin and data read, or the start of the error.
type Expected = Result<(&'static str, &'static [u8]), &'static str>;

#[test]
fn auth_switch_requests_are_read_or_refused() {
    // (packet, cleartext plugin enabled, expected)
    #[rustfmt::skip]
    let cases: [(&[u8], bool, Expected); 7] = [
        (b"\xfecaching_sha2_password\0abcdefghijabcdefghij\0", true, Ok(("caching_sha2_password", NONCE))),
        (b"\xfemysql_clear_password\0abcdefghijabcdefghij\0", true, Ok(("mysql_clear_password", NONCE))),
        // AWS Aurora's IAM authentication sends the cleartext switch with no data.
        (b"\xfemysql_clear_password\0", true, Ok(("mysql_clear_password", b""))),
        (
            b"\xfemysql_clear_password\0abcdefghijabcdefghij\0", false,
            Err("encountered unexpected or invalid data: mysql_cleartext_plugin disabled"),
        ),
        (
            b"\xfemysql_clear_password\0", false,
            Err("encountered unexpected or invalid data: mysql_cleartext_plugin disabled"),
        ),
        (
            b"\xfecaching_sha2_password\0abcdefghij\0", true,
            Err("encountered unexpected or invalid data: expected 21 bytes but found 11 bytes"),
        ),
        (
            b"\x00caching_sha2_password\0abcdefghijabcdefghij\0", true,
            Err("encountered unexpected or invalid data: expected 0xfe (AUTH_SWITCH) but found 0x0"),
        ),
    ];

    for (packet, cleartext, expected) in cases {
        let decoded = AuthSwitchRequest::decode_with(Bytes::from_static(packet), cleartext);
        match (decoded, expected) {
            (Ok(request), Ok((plugin, data))) => {
                assert_eq!(request.plugin.name(), plugin, "{packet:?}");
                assert_eq!(request.data, data, "{packet:?}");
            }
            // The message goes on with the module path, so only its start is fixed.
            (Err(error), Err(start)) => assert!(
                error.to_string().starts_with(start),
                "{packet:?}: expected an error starting with {start:?}, got {error}"
            ),
            (decoded, expected) => panic!("{packet:?}: got {decoded:?}, expected {expected:?}"),
        }
    }
}
