use super::*;

/// A notice body is a list of fields, each a type byte and a NUL-terminated string, ended by a
/// NUL. `V` is the severity in English; `S` the same, but translated by servers before 9.6, so
/// an `S` that is not English names no severity and the notice counts as `Log`.
/// <https://www.postgresql.org/docs/current/protocol-error-fields.html>
#[test]
fn notice_reads_severity_code_and_message() {
    let cases: [(&[u8], PgSeverity, &str, &str); 4] = [
        (
            b"SNOTICE\0VNOTICE\0C42710\0Mextension \"uuid-ossp\" already exists, skipping\0\
              Fextension.c\0L1656\0RCreateExtension\0\0",
            PgSeverity::Notice,
            "42710",
            "extension \"uuid-ossp\" already exists, skipping",
        ),
        (
            b"SFEHLER\0VERROR\0C42P01\0Mrelation \"x\" does not exist\0\0",
            PgSeverity::Error,
            "42P01",
            "relation \"x\" does not exist",
        ),
        (
            b"SWARNING\0C01000\0Mold server\0\0",
            PgSeverity::Warning,
            "01000",
            "old server",
        ),
        (
            b"SWARNUNG\0C01000\0Malter Server\0\0",
            PgSeverity::Log,
            "01000",
            "alter Server",
        ),
    ];

    for (body, severity, code, message) in cases {
        let notice = Notice::decode(Bytes::from_static(body)).unwrap();
        assert_eq!(
            (notice.severity(), notice.code(), notice.message()),
            (severity, code, message),
            "{body:?}"
        );
    }
}
