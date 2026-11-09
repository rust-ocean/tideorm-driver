use super::*;

#[test]
fn error_numbers_classify_what_their_sqlstate_does_not() {
    for (number, sqlstate, kind) in [
        (1062, "23000", ErrorKind::UniqueViolation),
        (1452, "23000", ErrorKind::ForeignKeyViolation),
        (1048, "23000", ErrorKind::NotNullViolation),
        (3819, "HY000", ErrorKind::CheckViolation),
        (4025, "23000", ErrorKind::CheckViolation),
        // MySQL's 4025 is another error.
        (4025, "HY000", ErrorKind::Other),
        (1205, "HY000", ErrorKind::LockNotAvailable),
        (3572, "HY000", ErrorKind::LockNotAvailable),
        // ER_LOCK_DEADLOCK's SQLSTATE is a serialization failure's.
        (1213, "40001", ErrorKind::Deadlock),
        (3024, "HY000", ErrorKind::StatementTimeout),
        (1969, "70100", ErrorKind::StatementTimeout),
        (1142, "42000", ErrorKind::InsufficientPrivilege),
        (1053, "08S01", ErrorKind::ConnectionClosed),
        (1927, "70100", ErrorKind::ConnectionClosed),
        (1366, "HY000", ErrorKind::InvalidValue),
        (1045, "28000", ErrorKind::AuthenticationFailed),
        (1698, "28000", ErrorKind::AuthenticationFailed),
        (1049, "42000", ErrorKind::UnknownDatabase),
    ] {
        assert_eq!(kind_of(number, Some(sqlstate)), kind, "{number}");
    }
}

#[test]
fn other_numbers_fall_back_to_their_sqlstate() {
    for (number, sqlstate, kind) in [
        (1406, Some("22001"), ErrorKind::InvalidValue),
        (1264, Some("22003"), ErrorKind::InvalidValue),
        (1064, Some("42000"), ErrorKind::SyntaxError),
        (1054, Some("42S22"), ErrorKind::UndefinedColumn),
        (1146, Some("42S02"), ErrorKind::UndefinedTable),
        (1043, Some("08S01"), ErrorKind::ConnectionClosed),
        (1105, Some("HY000"), ErrorKind::Other),
        (1105, None, ErrorKind::Other),
    ] {
        assert_eq!(kind_of(number, sqlstate), kind, "{number}");
    }
}
