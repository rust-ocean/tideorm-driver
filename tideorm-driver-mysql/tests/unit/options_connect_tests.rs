use crate::MySqlConnectOptions;

/// The `sql_mode` assignment every connection makes, adding `added`.
fn mode(added: &str) -> String {
    format!(
        "sql_mode=(SELECT TRIM(BOTH ',' FROM REPLACE(CONCAT(',', @@sql_mode, '{added},'), ',NO_BACKSLASH_ESCAPES,', ',')))"
    )
}

#[test]
fn a_new_connection_runs_one_set_for_the_options_it_has() {
    let defaults = MySqlConnectOptions::new;
    let nothing = || {
        defaults()
            .pipes_as_concat(false)
            .no_engine_substitution(false)
            .timezone(None)
            .set_names(false)
    };

    #[rustfmt::skip]
    let cases: [(&str, MySqlConnectOptions, String); 8] = [
        (
            "defaults",
            defaults(),
            format!("SET {},time_zone='+00:00',NAMES utf8mb4;", mode(",PIPES_AS_CONCAT,NO_ENGINE_SUBSTITUTION")),
        ),
        ("nothing else to set", nothing(), format!("SET {};", mode(""))),
        (
            "one SQL mode",
            nothing().no_engine_substitution(true),
            format!("SET {};", mode(",NO_ENGINE_SUBSTITUTION")),
        ),
        (
            "the other SQL mode",
            nothing().pipes_as_concat(true),
            format!("SET {};", mode(",PIPES_AS_CONCAT")),
        ),
        (
            "named time zone",
            nothing().timezone(Some("America/Port-au-Prince".to_owned())),
            format!("SET {},time_zone='America/Port-au-Prince';", mode("")),
        ),
        (
            "names with a collation",
            nothing().set_names(true).charset("utf8mb4").collation("utf8mb4_bin"),
            format!("SET {},NAMES utf8mb4 COLLATE utf8mb4_bin;", mode("")),
        ),
        (
            "names and time zone",
            nothing().set_names(true).charset("latin1").timezone(Some("+02:00".to_owned())),
            format!("SET {},time_zone='+02:00',NAMES latin1;", mode("")),
        ),
        (
            "names are not checked when not set",
            nothing().charset("not used;").set_names(false),
            format!("SET {};", mode("")),
        ),
    ];

    for (name, options, expected) in cases {
        assert_eq!(options.session_setup().unwrap(), expected, "{name}");
    }
}

#[test]
fn a_value_written_into_the_set_must_be_a_name() {
    let options = MySqlConnectOptions::new;
    for (name, options) in [
        (
            "time zone",
            options().timezone(Some("+00:00'; DROP TABLE t; --".into())),
        ),
        ("empty time zone", options().timezone(Some(String::new()))),
        ("character set", options().charset("utf8mb4; DROP TABLE t")),
        ("collation", options().collation("utf8mb4_bin COLLATE x")),
        ("quote", options().charset("utf8'")),
    ] {
        let error = options.session_setup().unwrap_err().to_string();
        assert!(error.contains("not a valid name"), "{name}: {error}");
        assert!(!error.contains("DROP"), "{name}: the value is not echoed");
    }
}
