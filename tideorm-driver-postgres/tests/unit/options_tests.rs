use super::*;

#[test]
fn test_options_formatting() {
    let options = PgConnectOptions::new().options([("geqo", "off")]);
    assert_eq!(options.options, Some("-c geqo=off".to_string()));
    let options = options.options([("search_path", "tideorm")]);
    assert_eq!(
        options.options,
        Some("-c geqo=off -c search_path=tideorm".to_string())
    );
    let options = PgConnectOptions::new().options([("geqo", "off"), ("statement_timeout", "5min")]);
    assert_eq!(
        options.options,
        Some("-c geqo=off -c statement_timeout=5min".to_string())
    );
    // https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-CONNECT-OPTIONS
    let options =
        PgConnectOptions::new().options([("application_name", r"/back\slash/ and\ spaces")]);
    assert_eq!(
        options.options,
        Some(r"-c application_name=/back\\slash/\ and\\\ spaces".to_string())
    );
    let options = PgConnectOptions::new();
    assert_eq!(options.options, None);
}

#[test]
fn test_pg_write_escaped() {
    let mut buf = String::new();
    let mut x = PgOptionsWriteEscaped(&mut buf);
    x.write_str("x").unwrap();
    x.write_str("").unwrap();
    x.write_char('\\').unwrap();
    x.write_str("y \\").unwrap();
    x.write_char(' ').unwrap();
    x.write_char('z').unwrap();
    assert_eq!(buf, r"x\\y\ \\\ z");
}
