use super::MySqlQueryResult;
use driver_core::database::QueryResult;

fn outcome<R: QueryResult>(result: &R) -> (u64, Option<i128>) {
    (result.rows_affected(), result.last_insert_id())
}

#[test]
fn generic_accessors_preserve_counts_and_identifiers() {
    let mut result = MySqlQueryResult {
        rows_affected: 2,
        last_insert_id: 0,
    };
    assert_eq!(outcome(&result), (2, Some(i128::from(0))));
    result.extend([MySqlQueryResult {
        rows_affected: 3,
        last_insert_id: u64::MAX,
    }]);
    assert_eq!(outcome(&result), (5, Some(i128::from(u64::MAX))));
    result.extend([]);
    assert_eq!(outcome(&result), (5, Some(i128::from(u64::MAX))));
}
