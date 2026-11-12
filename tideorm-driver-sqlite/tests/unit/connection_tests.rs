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

#[test]
fn flush_makes_the_transaction_state_current() {
    driver_core::rt::test_block_on(async {
        use driver_core::executor::Executor;
        use driver_core::transaction::TransactionState;

        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        conn.execute("BEGIN").await.unwrap();
        conn.flush().await.unwrap();
        assert_eq!(conn.transaction_state(), TransactionState::Active);

        // sent without waiting, as a pool return does
        conn.worker.start_rollback().unwrap();
        conn.flush().await.unwrap();
        assert_eq!(conn.transaction_state(), TransactionState::Idle);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !conn.worker.is_idle() {
            assert!(
                std::time::Instant::now() < deadline,
                "the worker never caught up"
            );
            driver_core::rt::sleep(std::time::Duration::from_millis(1)).await;
        }
        conn.close().await.unwrap();
    });
}

#[test]
fn a_cached_statement_reads_the_columns_a_schema_change_gave_it() {
    driver_core::rt::test_block_on(async {
        use driver_core::executor::Executor;
        use driver_core::row::Row;

        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        conn.execute("CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (1)")
            .await
            .unwrap();
        let read = || driver_core::query::query("SELECT * FROM t");
        assert_eq!(
            read().fetch_one(&mut conn).await.unwrap().columns().len(),
            1
        );

        // SQLite re-prepares the cached `SELECT *` with the new column
        conn.execute("ALTER TABLE t ADD COLUMN b TEXT DEFAULT 'x'")
            .await
            .unwrap();
        let row = read().fetch_one(&mut conn).await.unwrap();
        assert_eq!(row.columns().len(), 2);
        assert_eq!(row.get::<String, _>("b"), "x");
        conn.close().await.unwrap();
    });
}

/// Rows reach the caller in order and complete whatever the row buffer's size (0 hands each
/// batch over in a rendezvous), a statement after them still runs, and `fetch_optional` takes
/// the first row only.
#[test]
fn rows_arrive_in_order_whatever_the_row_buffer() {
    driver_core::rt::test_block_on(async {
        use driver_core::executor::Executor;
        use driver_core::row::Row;
        use futures_util::TryStreamExt;

        const ROWS: &str = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c \
                            WHERE x < 1000) SELECT x FROM c";
        for size in [0, 1, 50, 200] {
            let options: SqliteConnectOptions = "sqlite::memory:".parse().unwrap();
            let mut conn = SqliteConnection::connect_with(&options.row_buffer_size(size))
                .await
                .unwrap();
            let rows = driver_core::query::query(ROWS)
                .fetch_all(&mut conn)
                .await
                .unwrap();
            let values: Vec<i64> = rows.iter().map(|row| row.get(0)).collect();
            assert_eq!(values, (1..=1000).collect::<Vec<_>>(), "buffer {size}");

            let both: &'static str = format!("{ROWS}; SELECT 'after'").leak();
            let results: Vec<_> = (&mut conn).fetch_many(both).try_collect().await.unwrap();
            let last = results.iter().rev().find_map(|item| item.as_ref().right());
            assert_eq!(last.unwrap().get::<String, _>(0), "after", "buffer {size}");

            let first = driver_core::query::query(ROWS)
                .fetch_optional(&mut conn)
                .await
                .unwrap();
            assert_eq!(first.unwrap().get::<i64, _>(0), 1);
            conn.close().await.unwrap();
        }
    });
}
