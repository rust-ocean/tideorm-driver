use super::*;
use crate::SqliteConnection;
use driver_core::connection::Connection;
use driver_core::executor::Executor;
use driver_core::query::query;
use driver_core::row::Row;

async fn table() -> SqliteConnection {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
    conn.execute("CREATE TABLE t (a INTEGER, b INTEGER)")
        .await
        .unwrap();
    conn
}

async fn rows(conn: &mut SqliteConnection) -> i64 {
    query::<Sqlite>("SELECT COUNT(*) FROM t")
        .fetch_one(conn)
        .await
        .unwrap()
        .get(0)
}

fn refused(error: Error) {
    assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
}

/// A parameter with no value is an error, not a NULL, and nothing runs.
#[test]
fn a_parameter_without_a_value_is_refused() {
    driver_core::rt::test_block_on(async {
        let mut conn = table().await;
        for sql in [
            "INSERT INTO t VALUES (?, ?)",
            "INSERT INTO t VALUES (?1, ?2)",
        ] {
            let error = query::<Sqlite>(sql)
                .bind(1_i64)
                .execute(&mut conn)
                .await
                .unwrap_err();
            refused(error);
        }
        let error = query::<Sqlite>("INSERT INTO t VALUES (1, 2); INSERT INTO t VALUES (?, ?)")
            .execute(&mut conn)
            .await
            .unwrap_err();
        refused(error);
        // The first statement ran, as statements before a failing one do.
        assert_eq!(rows(&mut conn).await, 1);
    });
}

/// A parameter numbered 0, or named, is refused without stopping the
/// connection's worker.
#[test]
fn a_parameter_numbered_zero_is_refused() {
    driver_core::rt::test_block_on(async {
        let mut conn = table().await;
        for sql in ["SELECT $0", "SELECT $name", "SELECT :name"] {
            let error = query::<Sqlite>(sql)
                .bind(1_i64)
                .fetch_one(&mut conn)
                .await
                .unwrap_err();
            refused(error);
        }
        let one: i64 = query::<Sqlite>("SELECT 1")
            .fetch_one(&mut conn)
            .await
            .unwrap()
            .get(0);
        assert_eq!(one, 1);
    });
}

/// Values no parameter takes are refused before the last statement runs, and
/// after it when only a comment follows it.
#[test]
fn values_left_over_are_refused() {
    driver_core::rt::test_block_on(async {
        let mut conn = table().await;
        let error = query::<Sqlite>("INSERT INTO t VALUES (?, ?)")
            .bind(1_i64)
            .bind(2_i64)
            .bind(3_i64)
            .execute(&mut conn)
            .await
            .unwrap_err();
        refused(error);
        assert_eq!(rows(&mut conn).await, 0);

        let error = query::<Sqlite>("SELECT ?2")
            .bind(1_i64)
            .bind(2_i64)
            .bind(3_i64)
            .fetch_one(&mut conn)
            .await
            .unwrap_err();
        refused(error);

        let error = query::<Sqlite>("INSERT INTO t VALUES (?, ?) -- two")
            .bind(1_i64)
            .bind(2_i64)
            .bind(3_i64)
            .execute(&mut conn)
            .await
            .unwrap_err();
        refused(error);
    });
}

/// Numbered parameters take their values in any order and as often as they
/// appear, and anonymous ones carry on across statements.
#[test]
fn every_value_taken_is_accepted() {
    driver_core::rt::test_block_on(async {
        let mut conn = table().await;
        let row = query::<Sqlite>("SELECT ?2, ?1, ?1")
            .bind(1_i64)
            .bind(2_i64)
            .fetch_one(&mut conn)
            .await
            .unwrap();
        let values: (i64, i64, i64) = (row.get(0), row.get(1), row.get(2));
        assert_eq!(values, (2, 1, 1));

        let row = query::<Sqlite>("SELECT $1 + $2")
            .bind(3_i64)
            .bind(4_i64)
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(row.get::<i64, _>(0), 7);

        query::<Sqlite>("INSERT INTO t VALUES (?, ?); INSERT INTO t VALUES (?, ?);")
            .bind(1_i64)
            .bind(2_i64)
            .bind(3_i64)
            .bind(4_i64)
            .execute(&mut conn)
            .await
            .unwrap();
        let sum: i64 = query::<Sqlite>("SELECT SUM(a * b) FROM t")
            .fetch_one(&mut conn)
            .await
            .unwrap()
            .get(0);
        assert_eq!(sum, 2 + 12);
    });
}
