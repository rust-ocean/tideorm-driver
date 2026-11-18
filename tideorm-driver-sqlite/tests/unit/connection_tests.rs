use super::*;
use driver_core::connection::Connection;
use driver_core::query::query;

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

/// Two connections opened with one set of options for a shared in-memory database see one
/// database, however it is named; a `file:` URI keeps naming its file when it takes parameters.
#[test]
fn connections_to_a_shared_database_see_one_database() {
    driver_core::rt::test_block_on(async {
        let pid = std::process::id();
        let named = format!("sqlite:tideorm-shared-{pid}?mode=memory&cache=shared");
        let shared = SqliteConnectOptions::new()
            .filename(format!(":memory:{pid}"))
            .in_memory(true)
            .shared_cache(true);
        for options in [named.parse().unwrap(), shared] {
            let mut first = SqliteConnection::connect_with(&options).await.unwrap();
            first
                .execute("CREATE TABLE seen (v INTEGER)")
                .await
                .unwrap();
            let mut second = SqliteConnection::connect_with(&options).await.unwrap();
            let seen = query("SELECT count(*) FROM seen")
                .fetch_one(&mut second)
                .await;
            assert!(seen.is_ok(), "{options:?}: {seen:?}");
            second.close().await.unwrap();
            first.close().await.unwrap();
        }

        let file = std::env::temp_dir().join(format!("tideorm-uri-{pid}.db"));
        let options = SqliteConnectOptions::new()
            .filename(&file)
            .create_if_missing(true);
        let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
        conn.execute("CREATE TABLE kept (v INTEGER)").await.unwrap();
        conn.close().await.unwrap();
        let uri = format!("file:{}", file.display()).replace('\\', "/");
        let options = SqliteConnectOptions::new().filename(&uri).immutable(true);
        let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
        query("SELECT count(*) FROM kept")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        std::fs::remove_file(&file).unwrap();
    });
}

/// A double-quoted name no column has is an error, as on every other backend, not the text it
/// spells: a misspelt column would otherwise filter by, index or select a constant.
#[test]
fn a_quoted_name_no_column_has_is_an_error() {
    driver_core::rt::test_block_on(async {
        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        conn.execute("CREATE TABLE t (deleted_at TEXT); INSERT INTO t VALUES (NULL)")
            .await
            .unwrap();
        for sql in [
            "SELECT \"nope\" FROM t",
            "DELETE FROM t WHERE \"delted_at\" IS NULL",
            "CREATE INDEX i ON t (\"nope\")",
        ] {
            let error = conn.execute(sql).await.unwrap_err();
            assert!(
                error.to_string().contains("no such column"),
                "{sql}: {error}"
            );
        }
        let rows = conn
            .execute("DELETE FROM t WHERE \"deleted_at\" IS NULL AND 'text' = 'text'")
            .await
            .unwrap();
        assert_eq!(rows.rows_affected(), 1);
        conn.close().await.unwrap();
    });
}

/// A name two result columns share, a join's two `id`s, reads by position but not by name,
/// which would otherwise take whichever came last.
#[test]
fn a_name_two_result_columns_share_reads_only_by_position() {
    driver_core::rt::test_block_on(async {
        use driver_core::row::Row;

        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        let row = query("SELECT 1 AS id, 2 AS id, 3 AS other")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        let error = row.try_get::<i64, _>("id").unwrap_err();
        assert!(
            matches!(&error, driver_core::error::Error::ColumnDecode { index, .. } if index == "id"),
            "{error}"
        );
        assert_eq!(row.get::<i64, _>(0), 1);
        assert_eq!(row.get::<i64, _>(1), 2);
        assert_eq!(row.get::<i64, _>("other"), 3);
        conn.close().await.unwrap();
    });
}
