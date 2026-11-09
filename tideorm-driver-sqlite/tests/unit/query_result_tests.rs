use super::SqliteQueryResult;
use driver_core::database::QueryResult;

fn outcome<R: QueryResult>(result: &R) -> (u64, Option<i128>) {
    (result.rows_affected(), result.last_insert_id())
}

#[test]
fn generic_accessors_preserve_counts_and_identifiers() {
    let mut result = SqliteQueryResult {
        changes: 2,
        last_insert_rowid: i64::MIN,
    };
    assert_eq!(outcome(&result), (2, Some(i128::from(i64::MIN))));
    result.extend([SqliteQueryResult {
        changes: 3,
        last_insert_rowid: i64::MAX,
    }]);
    assert_eq!(outcome(&result), (5, Some(i128::from(i64::MAX))));
    result.extend([]);
    assert_eq!(outcome(&result), (5, Some(i128::from(i64::MAX))));
}
