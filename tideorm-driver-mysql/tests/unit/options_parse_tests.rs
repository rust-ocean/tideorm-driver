use super::*;

fn parse(url: &str) -> MySqlConnectOptions {
    url.parse().unwrap_or_else(|error| panic!("{url}: {error}"))
}

#[test]
fn it_parses_timezone() {
    for key in ["timezone", "time-zone"] {
        let options = parse(&format!(
            "mysql://user:password@hostname/database?{key}=%2B08:00"
        ));
        assert_eq!(options.timezone.as_deref(), Some("+08:00"), "{key}");
    }
}

#[test]
fn the_url_sets_the_connection_and_its_session() {
    let options = parse(
        "mysql://us%40er:p%40ss@db.example:3307/my%20db\
         ?charset=latin1&collation=latin1_bin&statement-cache-capacity=7&socket=/tmp/mysql.sock",
    );

    assert_eq!(options.host, "db.example");
    assert_eq!(options.port, 3307);
    assert_eq!(options.username, "us@er");
    assert_eq!(options.password.as_deref(), Some("p@ss"));
    assert_eq!(options.database.as_deref(), Some("my db"));
    assert_eq!(options.charset, "latin1");
    assert_eq!(options.collation.as_deref(), Some("latin1_bin"));
    assert_eq!(options.statement_cache_capacity, 7);
    assert_eq!(
        options.socket.as_deref(),
        Some(std::path::Path::new("/tmp/mysql.sock"))
    );
}

#[test]
fn ssl_mode_takes_mysqls_names_in_any_case() {
    let cases = [
        ("sslmode=disabled", "Disabled"),
        ("ssl-mode=PREFERRED", "Preferred"),
        ("ssl-mode=Required", "Required"),
        ("sslmode=verify_ca", "VerifyCa"),
        ("ssl-mode=VERIFY_IDENTITY", "VerifyIdentity"),
    ];

    for (query, mode) in cases {
        let options = parse(&format!("mysql://localhost/db?{query}"));
        assert_eq!(format!("{:?}", options.ssl_mode), mode, "{query}");
    }

    for query in [
        "ssl-mode=verify-ca",
        "ssl-mode=",
        "statement-cache-capacity=many",
    ] {
        let url = format!("mysql://localhost/db?{query}");
        let error = url.parse::<MySqlConnectOptions>().unwrap_err();
        assert!(matches!(error, Error::Configuration(_)), "{query}: {error}");
    }
}

#[test]
fn debug_leaves_the_password_and_client_key_out() {
    let options =
        parse("mysql://app:s3cret-pw@db/app").ssl_client_key_from_pem("PRIVATE-KEY-BYTES");
    let shown = format!("{options:?}");
    assert!(shown.contains("password: Some(\"<redacted>\")"), "{shown}");
    assert!(!shown.contains("s3cret-pw"), "{shown}");
    assert!(!shown.contains("80, 82, 73"), "the key's bytes: {shown}");
}
