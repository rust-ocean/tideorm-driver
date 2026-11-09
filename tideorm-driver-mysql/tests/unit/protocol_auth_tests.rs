use super::*;

#[test]
fn each_plugin_is_read_back_from_its_name() {
    for plugin in AuthPlugin::ALL {
        let parsed: AuthPlugin = plugin.name().parse().unwrap();
        assert_eq!(parsed.name(), plugin.name());
    }
}

#[test]
fn unknown_or_differently_cased_names_are_refused() {
    for name in [
        "",
        "ed25519",
        "MYSQL_NATIVE_PASSWORD",
        "mysql_native_password ",
    ] {
        let error = name.parse::<AuthPlugin>().unwrap_err();
        assert!(matches!(error, Error::Protocol(_)), "{name:?}: {error}");
    }
}
