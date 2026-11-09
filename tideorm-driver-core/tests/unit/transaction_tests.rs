use super::*;

/// What a begin cancelled at `depth` queues: its own level, a savepoint
/// released with it.
#[test]
fn a_level_is_undone_at_its_own_depth() {
    assert_eq!(rollback_ansi_transaction_sql(1).as_str(), "ROLLBACK");
    assert_eq!(
        rollback_ansi_transaction_sql(2).as_str(),
        "ROLLBACK TO SAVEPOINT _tideorm_savepoint_1; RELEASE SAVEPOINT _tideorm_savepoint_1"
    );
    assert_eq!(
        rollback_ansi_transaction_sql(4).as_str(),
        "ROLLBACK TO SAVEPOINT _tideorm_savepoint_3; RELEASE SAVEPOINT _tideorm_savepoint_3"
    );
}

/// A custom statement opens only a transaction; a nested level is a savepoint.
#[test]
fn a_custom_begin_opens_only_the_outer_level() {
    assert_eq!(begin_statement(0, None).unwrap().as_str(), "BEGIN");
    assert_eq!(
        begin_statement(1, None).unwrap().as_str(),
        "SAVEPOINT _tideorm_savepoint_1"
    );
    let custom = || Some("BEGIN IMMEDIATE".into_sql_str());
    assert_eq!(
        begin_statement(0, custom()).unwrap().as_str(),
        "BEGIN IMMEDIATE"
    );
    assert!(matches!(
        begin_statement(1, custom()),
        Err(Error::InvalidSavePointStatement)
    ));
}
