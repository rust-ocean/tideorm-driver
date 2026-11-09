use crate::MySqlConnectOptions;

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
    let cases: [(&str, MySqlConnectOptions, Option<&str>); 7] = [
        (
            "defaults",
            defaults(),
            Some("SET sql_mode=(SELECT CONCAT(@@sql_mode, ',PIPES_AS_CONCAT,NO_ENGINE_SUBSTITUTION')),time_zone='+00:00',NAMES utf8mb4;"),
        ),
        ("nothing to set", nothing(), None),
        (
            "one SQL mode",
            nothing().no_engine_substitution(true),
            Some("SET sql_mode=(SELECT CONCAT(@@sql_mode, ',NO_ENGINE_SUBSTITUTION'));"),
        ),
        (
            "the other SQL mode",
            nothing().pipes_as_concat(true),
            Some("SET sql_mode=(SELECT CONCAT(@@sql_mode, ',PIPES_AS_CONCAT'));"),
        ),
        (
            "time zone only",
            nothing().timezone(Some("Europe/Berlin".to_owned())),
            Some("SET time_zone='Europe/Berlin';"),
        ),
        (
            "names with a collation",
            nothing().set_names(true).charset("utf8mb4").collation("utf8mb4_bin"),
            Some("SET NAMES utf8mb4 COLLATE utf8mb4_bin;"),
        ),
        (
            "names and time zone",
            nothing().set_names(true).charset("latin1").timezone(Some("+02:00".to_owned())),
            Some("SET time_zone='+02:00',NAMES latin1;"),
        ),
    ];

    for (name, options, expected) in cases {
        assert_eq!(options.session_setup().as_deref(), expected, "{name}");
    }
}
