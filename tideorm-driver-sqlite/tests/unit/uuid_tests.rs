use super::*;
use crate::SqliteConnection;
use driver_core::connection::Connection;
use driver_core::row::Row;

const ID: &str = "67e55044-10b1-426f-9247-bb680e5fe0c8";

/// The `Uuid` decoded from the one value `sql` returns.
fn uuid_of(sql: &'static str) -> Result<Uuid, driver_core::error::Error> {
    driver_core::rt::test_block_on(async {
        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        let row = driver_core::query::query(sql)
            .fetch_one(&mut conn)
            .await
            .unwrap();
        row.try_get::<Uuid, _>(0)
    })
}

#[test]
fn a_uuid_reads_from_its_bytes_and_from_its_text() {
    let id = Uuid::parse_str(ID).unwrap();
    assert_eq!(
        uuid_of("SELECT x'67e5504410b1426f9247bb680e5fe0c8'").unwrap(),
        id
    );
    assert_eq!(
        uuid_of("SELECT '67e55044-10b1-426f-9247-bb680e5fe0c8'").unwrap(),
        id
    );
    assert_eq!(
        uuid_of("SELECT '67e5504410b1426f9247bb680e5fe0c8'").unwrap(),
        id
    );
}

#[test]
fn text_or_bytes_that_hold_no_uuid_fail() {
    // Sixteen characters are not sixteen bytes.
    assert!(uuid_of("SELECT '0123456789abcdef'").is_err());
    assert!(uuid_of("SELECT x'0102'").is_err());
}
