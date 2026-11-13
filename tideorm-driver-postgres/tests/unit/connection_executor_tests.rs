use super::*;

/// A cached statement is reused only for arguments of its parameters' types: one parsed
/// for an `INT4` would read an `INT8`'s eight bytes as a malformed `INT4`.
#[test]
fn a_statement_takes_only_its_own_parameter_types() {
    type T = PgTypeInfo;

    assert!(takes_types(&[T::INT4], &[T::INT4]));
    assert!(!takes_types(&[T::INT4], &[T::INT8]));
    assert!(!takes_types(&[T::INT8, T::INT4], &[T::INT8, T::INT8]));
    // a type declared by name has no OID until resolved; patching its holes checks it
    assert!(takes_types(&[T::INT4], &[T::with_name("mood")]));
    assert!(takes_types(&[], &[]));
}
