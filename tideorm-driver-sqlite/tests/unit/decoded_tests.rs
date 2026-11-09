use super::*;
use crate::SqliteConnection;
use driver_core::connection::Connection;
use driver_core::executor::Executor;
use driver_core::row::Row;
use SqliteDecodedValue as V;

/// Each column of the one row `sql` returns from a table of declared types.
fn decoded(sql: &'static str) -> Vec<SqliteDecodedValue> {
    driver_core::rt::test_block_on(async {
        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        conn.execute(
            "CREATE TABLE t (b BOOLEAN, d DATE, tm TIME, at DATETIME, price DECIMAL, \
             doc JSON, n INTEGER, s TEXT, bytes BLOB)",
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO t VALUES \
             (1, '2024-02-29', '10:30:00', 0, 1.5, '[1]', '00123', '00123', x'0102'), \
             ('yes', 'soon', 'noon', 'later', 'n/a', 'x', 'y', 'z', 'w')",
        )
        .await
        .unwrap();
        let row = driver_core::query::query(sql)
            .fetch_one(&mut conn)
            .await
            .unwrap();
        (0..row.len())
            .map(|index| row.try_get_raw(index).unwrap().decode_lossless().unwrap())
            .collect()
    })
}

#[test]
fn declared_columns_read_the_values_of_their_form() {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .unwrap();
    assert_eq!(
        decoded("SELECT * FROM t WHERE rowid = 1"),
        vec![
            V::Bool(true),
            V::Date(NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()),
            V::Time(NaiveTime::from_hms_opt(10, 30, 0).unwrap()),
            V::DateTime(epoch),
            // SQLite names no type for `DECIMAL` or `JSON`: read as stored.
            V::Real(1.5),
            V::Text("[1]".into()),
            // INTEGER affinity stores numeric text as an integer.
            V::Integer(123),
            V::Text("00123".into()),
            V::Blob(vec![1, 2]),
        ]
    );
}

#[test]
fn values_not_of_the_declared_form_read_as_stored() {
    assert_eq!(
        decoded("SELECT b, d, tm, at, price FROM t WHERE rowid = 2"),
        ["yes", "soon", "noon", "later", "n/a"]
            .map(|text| V::Text(text.into()))
            .to_vec()
    );
}

#[test]
fn expressions_read_as_stored() {
    assert_eq!(
        decoded("SELECT b + 1, d || '!', NULL, 2.5 * 2, 'true' FROM t WHERE rowid = 1"),
        vec![
            V::Integer(2),
            V::Text("2024-02-29!".into()),
            V::Null,
            V::Real(5.0),
            V::Text("true".into()),
        ]
    );
}
