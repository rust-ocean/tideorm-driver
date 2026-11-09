use super::PgQueryResult;
use driver_core::database::QueryResult;

fn outcome<R: QueryResult>(result: &R) -> (u64, Option<i128>) {
    (result.rows_affected(), result.last_insert_id())
}

#[test]
fn generic_accessors_preserve_counts_and_identifiers() {
    let mut result = PgQueryResult { rows_affected: 2 };
    assert_eq!(outcome(&result), (2, None));
    result.extend([PgQueryResult { rows_affected: 3 }]);
    assert_eq!(outcome(&result), (5, None));
    result.extend([]);
    assert_eq!(outcome(&result), (5, None));
}
