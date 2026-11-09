use super::*;
use crate::test_support::{binary, text};

#[test]
fn every_integer_type_but_i8_takes_every_integer_column() {
    for ty in [
        PgTypeInfo::INT2,
        PgTypeInfo::INT4,
        PgTypeInfo::INT8,
        PgTypeInfo::OID,
    ] {
        assert!(<i16 as Type<Postgres>>::compatible(&ty), "{ty:?}");
        assert!(<i32 as Type<Postgres>>::compatible(&ty), "{ty:?}");
        assert!(<i64 as Type<Postgres>>::compatible(&ty), "{ty:?}");
        assert!(<u16 as Type<Postgres>>::compatible(&ty), "{ty:?}");
        assert!(<u64 as Type<Postgres>>::compatible(&ty), "{ty:?}");
        assert!(!<i8 as Type<Postgres>>::compatible(&ty), "{ty:?}");
    }
    for ty in [
        PgTypeInfo::INT2_ARRAY,
        PgTypeInfo::INT4_ARRAY,
        PgTypeInfo::INT8_ARRAY,
        PgTypeInfo::OID_ARRAY,
    ] {
        assert!(<Vec<i64> as Type<Postgres>>::compatible(&ty), "{ty:?}");
        assert!(<Vec<u32> as Type<Postgres>>::compatible(&ty), "{ty:?}");
        assert!(<Vec<u16> as Type<Postgres>>::compatible(&ty), "{ty:?}");
    }
    assert!(!<i64 as Type<Postgres>>::compatible(&PgTypeInfo::TEXT));
    assert!(!<i64 as Type<Postgres>>::compatible(&PgTypeInfo::NUMERIC));
}

#[test]
fn values_are_range_checked() {
    let int4 = 40_000i32.to_be_bytes();
    assert_eq!(
        i64::decode(binary(&int4, PgTypeInfo::INT4)).unwrap(),
        40_000
    );
    assert_eq!(
        u16::decode(binary(&int4, PgTypeInfo::INT4)).unwrap(),
        40_000
    );
    assert!(i16::decode(binary(&int4, PgTypeInfo::INT4)).is_err());

    let negative = (-1i16).to_be_bytes();
    assert_eq!(
        i64::decode(binary(&negative, PgTypeInfo::INT2)).unwrap(),
        -1
    );
    assert!(u32::decode(binary(&negative, PgTypeInfo::INT2)).is_err());

    assert_eq!(i32::decode(text("123", PgTypeInfo::INT8)).unwrap(), 123);
}

#[test]
fn an_oid_is_unsigned() {
    let oid = 4_000_000_000u32.to_be_bytes();
    assert_eq!(
        i64::decode(binary(&oid, PgTypeInfo::OID)).unwrap(),
        4_000_000_000
    );
    assert_eq!(
        u32::decode(binary(&oid, PgTypeInfo::OID)).unwrap(),
        4_000_000_000
    );
    assert!(i32::decode(binary(&oid, PgTypeInfo::OID)).is_err());
}

#[test]
fn integer_arrays_read_into_any_width() {
    let values =
        <Vec<i64> as Decode<Postgres>>::decode(text("{1,-2,3}", PgTypeInfo::INT4_ARRAY)).unwrap();
    assert_eq!(values, [1, -2, 3]);
    let values =
        <Vec<u32> as Decode<Postgres>>::decode(text("{7,4000000000}", PgTypeInfo::INT8_ARRAY))
            .unwrap();
    assert_eq!(values, [7, 4_000_000_000]);
    assert!(
        <Vec<u16> as Decode<Postgres>>::decode(text("{70000}", PgTypeInfo::INT4_ARRAY)).is_err()
    );
}
