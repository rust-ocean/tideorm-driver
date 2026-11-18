use super::*;
use crate::SqliteConnection;
use driver_core::connection::Connection;
use driver_core::query::query;
use driver_core::row::Row;

/// A Julian day reads as the moment SQLite's own `datetime()` reads, to the millisecond, before
/// 1970 too.
#[test]
fn a_julian_day_reads_as_sqlite_reads_it() {
    driver_core::rt::test_block_on(async {
        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        for moment in [
            "2024-03-10 02:30:00.000",
            "2024-06-15 12:34:56.789",
            "1969-12-31 23:59:59.500",
            "1900-01-01 00:00:00.250",
            "0100-07-04 18:00:00.001",
        ] {
            let row = query::<Sqlite>("SELECT julianday(?1), datetime(julianday(?1), 'subsec')")
                .bind(moment)
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert_eq!(row.get::<String, _>(1), moment);
            let read: NaiveDateTime = row.get(0);
            assert_eq!(read.format("%F %T%.3f").to_string(), moment);
            assert_eq!(
                read.and_utc().timestamp_subsec_nanos() % 1_000_000,
                0,
                "{moment}"
            );
        }
    });
}
