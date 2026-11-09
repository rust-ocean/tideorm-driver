use super::*;

#[test]
fn result_codes_and_messages_classify_the_error() {
    for (code, message, kind) in [
        (
            2067,
            "UNIQUE constraint failed: t.a",
            ErrorKind::UniqueViolation,
        ),
        (
            1555,
            "UNIQUE constraint failed: t.id",
            ErrorKind::UniqueViolation,
        ),
        (
            787,
            "FOREIGN KEY constraint failed",
            ErrorKind::ForeignKeyViolation,
        ),
        (
            1299,
            "NOT NULL constraint failed: t.a",
            ErrorKind::NotNullViolation,
        ),
        (
            275,
            "CHECK constraint failed: a > 0",
            ErrorKind::CheckViolation,
        ),
        (1, "no such table: users", ErrorKind::UndefinedTable),
        (1, "no such column: nme", ErrorKind::UndefinedColumn),
        (
            1,
            "table users has no column named nme",
            ErrorKind::UndefinedColumn,
        ),
        (1, "near \"SELEC\": syntax error", ErrorKind::SyntaxError),
        (5, "database is locked", ErrorKind::LockNotAvailable),
        // SQLITE_BUSY_SNAPSHOT, SQLITE_BUSY_TIMEOUT
        (517, "database is locked", ErrorKind::LockNotAvailable),
        (773, "database is locked", ErrorKind::LockNotAvailable),
        (6, "database table is locked", ErrorKind::LockNotAvailable),
        (262, "database table is locked", ErrorKind::LockNotAvailable),
        (1, "some other failure", ErrorKind::Other),
        // A message is read only for the generic SQLITE_ERROR.
        (19, "no such table: users", ErrorKind::Other),
    ] {
        assert_eq!(kind_of(code, message), kind, "{code} {message}");
    }
}
