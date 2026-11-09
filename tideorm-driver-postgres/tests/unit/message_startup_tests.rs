use super::*;

/// The length (counting itself), protocol version 3.0, then name/value pairs, each string
/// NUL-terminated, and a final NUL.
#[test]
fn startup_encodes_version_and_parameters() {
    let cases: [(Startup<'_>, &[u8]); 3] = [
        (
            Startup {
                username: Some("postgres"),
                database: Some("postgres"),
                params: &[],
            },
            b"\0\0\0)\0\x03\0\0user\0postgres\0database\0postgres\0\0",
        ),
        (
            Startup {
                username: Some("u"),
                database: None,
                params: &[("application_name", "x")],
            },
            b"\0\0\0#\0\x03\0\0user\0u\0application_name\0x\0\0",
        ),
        (
            Startup {
                username: None,
                database: None,
                params: &[],
            },
            b"\0\0\0\x09\0\x03\0\0\0",
        ),
    ];

    for (message, expected) in cases {
        let mut buf = Vec::new();
        message.encode(&mut buf).unwrap();
        assert_eq!(buf, expected);
    }
}
