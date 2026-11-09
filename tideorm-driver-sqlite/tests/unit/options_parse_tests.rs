use super::*;

#[test]
fn test_parse_in_memory() -> Result<(), Error> {
    let options: SqliteConnectOptions = "sqlite::memory:".parse()?;
    assert!(options.in_memory);
    assert!(options.shared_cache);

    let options: SqliteConnectOptions = "sqlite://?mode=memory".parse()?;
    assert!(options.in_memory);
    assert!(options.shared_cache);

    let options: SqliteConnectOptions = "sqlite://:memory:".parse()?;
    assert!(options.in_memory);
    assert!(options.shared_cache);

    let options: SqliteConnectOptions = "sqlite://?mode=memory&cache=private".parse()?;
    assert!(options.in_memory);
    assert!(!options.shared_cache);

    let options: SqliteConnectOptions = "SQLite::memory:".parse()?;
    assert!(options.in_memory);

    Ok(())
}

#[test]
fn test_parse_strips_the_scheme_once() -> Result<(), Error> {
    let options: SqliteConnectOptions = "sqlite:///tmp/a.db".parse()?;
    assert_eq!(&*options.filename.to_string_lossy(), "/tmp/a.db");

    let options: SqliteConnectOptions = "sqlite:sqlite:a.db".parse()?;
    assert_eq!(&*options.filename.to_string_lossy(), "sqlite:a.db");

    Ok(())
}

#[test]
fn test_parse_read_only() -> Result<(), Error> {
    let options: SqliteConnectOptions = "sqlite://a.db?mode=ro".parse()?;
    assert!(options.read_only);
    assert_eq!(&*options.filename.to_string_lossy(), "a.db");

    Ok(())
}

#[test]
fn the_last_mode_wins_as_in_sqlite() -> Result<(), Error> {
    for (url, read_only, creates) in [
        ("sqlite://a.db", false, false),
        ("sqlite://a.db?mode=rwc", false, true),
        ("sqlite://a.db?mode=rwc&mode=rw", false, false),
        ("sqlite://a.db?mode=ro&mode=rwc", false, true),
        ("sqlite://a.db?mode=rwc&mode=ro", true, false),
    ] {
        let options: SqliteConnectOptions = url.parse()?;
        assert_eq!(options.is_read_only(), read_only, "{url}");
        assert_eq!(options.get_create_if_missing(), creates, "{url}");
    }

    Ok(())
}

#[test]
fn test_parse_shared_in_memory() -> Result<(), Error> {
    let options: SqliteConnectOptions = "sqlite://a.db?cache=shared".parse()?;
    assert!(options.shared_cache);
    assert_eq!(&*options.filename.to_string_lossy(), "a.db");

    Ok(())
}

#[test]
fn transient_and_private_databases_are_told_apart() -> Result<(), Error> {
    for (url, transient, private) in [
        ("sqlite::memory:", true, false),
        // An empty name opens a temporary database, with or without `mode=memory`.
        ("sqlite://?mode=memory", true, true),
        ("sqlite://a.db?mode=memory", true, false),
        ("sqlite::memory:?cache=private", true, true),
        ("sqlite://a.db?mode=memory&cache=private", true, true),
        ("sqlite://", true, true),
        ("sqlite://a.db", false, false),
        ("sqlite://a.db?cache=private", false, false),
    ] {
        let options: SqliteConnectOptions = url.parse()?;
        assert_eq!(options.is_transient(), transient, "{url}");
        assert_eq!(options.is_connection_private(), private, "{url}");
    }
    // SQLite's own `:memory:` name, the default filename, is in memory too.
    let options = SqliteConnectOptions::new();
    assert!(options.is_transient() && options.is_connection_private());
    let options = SqliteConnectOptions::new().shared_cache(true);
    assert!(options.is_transient() && !options.is_connection_private());
    Ok(())
}
