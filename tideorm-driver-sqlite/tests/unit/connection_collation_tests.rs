use super::*;
use crate::{Sqlite, SqliteConnectOptions, SqliteConnection};
use driver_core::connection::Connection;
use driver_core::query::query;
use driver_core::row::Row;
use std::str::FromStr;

/// Shorter first, unlike SQLite's own `BINARY` order.
fn by_length(a: &str, b: &str) -> Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

async fn sorted(conn: &mut SqliteConnection) -> Vec<String> {
    query::<Sqlite>(
        "SELECT column1 FROM (VALUES ('ccc'), ('a'), ('bb')) ORDER BY column1 COLLATE custom",
    )
    .fetch_all(conn)
    .await
    .unwrap()
    .iter()
    .map(|row| row.get(0))
    .collect()
}

fn options() -> SqliteConnectOptions {
    SqliteConnectOptions::from_str("sqlite::memory:").unwrap()
}

#[test]
fn a_registered_collation_orders_rows() {
    driver_core::rt::test_block_on(async {
        // Through the connect options, and on an open connection.
        let mut conn = SqliteConnection::connect_with(&options().collation("custom", by_length))
            .await
            .unwrap();
        assert_eq!(sorted(&mut conn).await, ["a", "bb", "ccc"]);

        // Registering the name again replaces the closure, and SQLite releases the old one.
        conn.lock_handle()
            .await
            .unwrap()
            .create_collation("custom", |a, b| by_length(b, a))
            .unwrap();
        assert_eq!(sorted(&mut conn).await, ["ccc", "bb", "a"]);
    });
}

#[test]
fn a_collation_sees_invalid_utf8_replaced() {
    driver_core::rt::test_block_on(async {
        let options = options().collation("custom", |a: &str, b: &str| a.cmp(b));
        let mut conn = SqliteConnection::connect_with(&options).await.unwrap();

        let (custom, binary): (bool, bool) = query::<Sqlite>(
            "SELECT CAST(x'ff' AS TEXT) = ?1 COLLATE custom, CAST(x'ff' AS TEXT) = ?1",
        )
        .bind("\u{FFFD}")
        .fetch_one(&mut conn)
        .await
        .map(|row| (row.get(0), row.get(1)))
        .unwrap();
        assert!(custom);
        assert!(!binary);
    });
}

/// The name is checked before the closure is handed over, so a refused registration
/// keeps no copy of it.
#[test]
fn a_name_with_a_nul_byte_is_refused() {
    driver_core::rt::test_block_on(async {
        let refused = |error: Error| match error {
            Error::Protocol(message) => {
                assert!(
                    message.starts_with(r#"invalid collation name: "bad\0""#),
                    "{message}"
                )
            }
            other => panic!("{other:?}"),
        };
        let captured = Arc::new(());
        let capturing = || {
            let captured = Arc::clone(&captured);
            move |a: &str, b: &str| {
                let _ = &captured;
                a.cmp(b)
            }
        };

        let options = options().collation("bad\0", capturing());
        refused(SqliteConnection::connect_with(&options).await.unwrap_err());
        drop(options);
        assert_eq!(Arc::strong_count(&captured), 1, "through the options");

        let mut conn = SqliteConnection::connect_with(&self::options())
            .await
            .unwrap();
        let mut locked = conn.lock_handle().await.unwrap();
        refused(locked.create_collation("bad\0", capturing()).unwrap_err());
        assert_eq!(Arc::strong_count(&captured), 1, "on an open connection");
    });
}

/// A comparator that panics aborts the process, as the public docs say, rather than
/// unwinding into SQLite. The panic runs in a child copy of this test binary.
#[test]
fn a_panicking_collation_aborts_the_process() {
    const CHILD: &str = "TIDEORM_COLLATION_PANIC_CHILD";
    if std::env::var_os(CHILD).is_some() {
        driver_core::rt::test_block_on(async {
            let options = options().collation("custom", |_: &str, _: &str| -> Ordering {
                panic!("comparator panicked")
            });
            let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
            sorted(&mut conn).await;
        });
        return;
    }

    let (_, module) = module_path!().split_once("::").unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "--nocapture", "--test-threads=1"])
        .arg(format!(
            "{module}::a_panicking_collation_aborts_the_process"
        ))
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("comparator panicked"), "{stderr}");
    // 101 is the test harness reporting a panic it caught; an abort ends the process
    // without it.
    assert!(!output.status.success(), "{stderr}");
    assert_ne!(output.status.code(), Some(101), "{stderr}");
}
