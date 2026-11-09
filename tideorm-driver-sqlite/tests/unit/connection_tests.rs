use super::*;
use driver_core::connection::Connection;

#[test]
fn main_file_is_where_sqlite_opened_the_database() {
    driver_core::rt::test_block_on(async {
        for url in ["sqlite::memory:", "sqlite://?mode=memory", "sqlite://"] {
            let conn = SqliteConnection::connect(url).await.unwrap();
            assert_eq!(conn.main_file(), None, "{url}");
            conn.close().await.unwrap();
        }

        let file =
            std::env::temp_dir().join(format!("tideorm-main-file-{}.db", std::process::id()));
        let options = SqliteConnectOptions::new()
            .filename(&file)
            .create_if_missing(true);
        let conn = SqliteConnection::connect_with(&options).await.unwrap();
        let main = conn.main_file().expect("a file database has a main file");
        assert!(main.is_absolute(), "{main:?}");
        assert_eq!(
            std::fs::canonicalize(main).unwrap(),
            std::fs::canonicalize(&file).unwrap()
        );
        conn.close().await.unwrap();
        std::fs::remove_file(&file).unwrap();
    });
}

#[test]
fn options_note_an_attach_their_connections_prepare() {
    driver_core::rt::test_block_on(async {
        use driver_core::executor::Executor;

        let options: SqliteConnectOptions = "sqlite::memory:".parse().unwrap();
        let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
        conn.execute("SELECT 1").await.unwrap();
        assert!(!options.has_attached());

        let session = options.clone();
        conn.execute("ATTACH DATABASE ':memory:' AS other")
            .await
            .unwrap();
        assert!(options.has_attached());
        // A clone shares the note, as a pool's connections do.
        assert!(session.has_attached());
        conn.execute("DETACH DATABASE other").await.unwrap();
        assert!(options.has_attached());
        conn.close().await.unwrap();

        let fresh: SqliteConnectOptions = "sqlite::memory:".parse().unwrap();
        assert!(!fresh.has_attached());
    });
}

#[test]
fn tideorm_raise_fails_its_statement_with_the_message() {
    driver_core::rt::test_block_on(async {
        use driver_core::executor::Executor;

        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        let error = conn
            .execute("SELECT CASE WHEN 1 THEN tideorm_raise('fixture check failed') END")
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("fixture check failed"),
            "{error}"
        );
        // Not raised when not reached.
        conn.execute("SELECT CASE WHEN 0 THEN tideorm_raise('unreached') END")
            .await
            .unwrap();
        // Top-level SQL only: a view could not be read where it is missing.
        conn.execute("CREATE VIEW raising AS SELECT tideorm_raise('in a view')")
            .await
            .unwrap();
        let error = conn.execute("SELECT * FROM raising").await.unwrap_err();
        assert!(error.to_string().contains("unsafe use"), "{error}");
        conn.close().await.unwrap();
    });
}
