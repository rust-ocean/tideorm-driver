use super::*;

#[test]
fn sqlstates_classify_the_error() {
    for (sqlstate, kind) in [
        ("23505", ErrorKind::UniqueViolation),
        ("23503", ErrorKind::ForeignKeyViolation),
        ("23502", ErrorKind::NotNullViolation),
        ("23514", ErrorKind::CheckViolation),
        ("23P01", ErrorKind::ExclusionViolation),
        // A value that does not fit its column (too long, out of range, a bad
        // datetime, invalid text for the type) is the caller's input to fix.
        ("22001", ErrorKind::InvalidValue),
        ("22003", ErrorKind::InvalidValue),
        ("22007", ErrorKind::InvalidValue),
        ("22P02", ErrorKind::InvalidValue),
        ("42601", ErrorKind::SyntaxError),
        ("42703", ErrorKind::UndefinedColumn),
        ("42P01", ErrorKind::UndefinedTable),
        ("42501", ErrorKind::InsufficientPrivilege),
        ("28P01", ErrorKind::AuthenticationFailed),
        ("28000", ErrorKind::AuthenticationFailed),
        ("3D000", ErrorKind::UnknownDatabase),
        ("40P01", ErrorKind::Deadlock),
        ("40001", ErrorKind::SerializationFailure),
        ("55P03", ErrorKind::LockNotAvailable),
        ("57014", ErrorKind::StatementTimeout),
        ("53300", ErrorKind::TooManyConnections),
        ("08006", ErrorKind::ConnectionClosed),
        // A Bind with the wrong number of values; the connection goes on.
        ("08P01", ErrorKind::Other),
        ("57P01", ErrorKind::ConnectionClosed),
        // The server ends the connection on its own idle, idle-in-transaction
        // and transaction timeouts.
        ("57P05", ErrorKind::ConnectionClosed),
        ("25P03", ErrorKind::ConnectionClosed),
        ("25P04", ErrorKind::ConnectionClosed),
        ("XX000", ErrorKind::Other),
    ] {
        assert_eq!(kind_of(sqlstate), kind, "{sqlstate}");
    }
}
