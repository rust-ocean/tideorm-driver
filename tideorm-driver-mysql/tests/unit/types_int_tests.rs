use super::*;

fn column(ty: ColumnType, unsigned: bool) -> MySqlTypeInfo {
    let mut info = MySqlTypeInfo::binary(ty);
    if unsigned {
        info.flags |= ColumnFlags::UNSIGNED;
    }
    info
}

fn binary(bytes: &[u8], type_info: MySqlTypeInfo) -> MySqlValueRef<'_> {
    MySqlValueRef {
        value: Some(bytes),
        row: None,
        type_info,
        format: MySqlValueFormat::Binary,
    }
}

#[test]
fn every_integer_type_takes_every_integer_column() {
    for ty in [
        ColumnType::Tiny,
        ColumnType::Short,
        ColumnType::Long,
        ColumnType::Int24,
        ColumnType::LongLong,
        ColumnType::Year,
        ColumnType::Bit,
    ] {
        for unsigned in [false, true] {
            let info = column(ty, unsigned);
            assert!(<i8 as Type<MySql>>::compatible(&info), "{ty:?}");
            assert!(<i64 as Type<MySql>>::compatible(&info), "{ty:?}");
            assert!(<u8 as Type<MySql>>::compatible(&info), "{ty:?}");
            assert!(<u64 as Type<MySql>>::compatible(&info), "{ty:?}");
        }
    }
    let text = MySqlTypeInfo::binary(ColumnType::VarString);
    assert!(!<i64 as Type<MySql>>::compatible(&text));
    assert!(!<u64 as Type<MySql>>::compatible(&text));
}

#[test]
fn bytes_are_read_by_the_columns_signedness() {
    let unsigned_tiny = column(ColumnType::Tiny, true);
    assert_eq!(
        i64::decode(binary(&[0xFF], unsigned_tiny.clone())).unwrap(),
        255
    );
    assert!(i8::decode(binary(&[0xFF], unsigned_tiny.clone())).is_err());
    assert_eq!(u8::decode(binary(&[0xFF], unsigned_tiny)).unwrap(), 255);

    let signed_tiny = column(ColumnType::Tiny, false);
    assert_eq!(
        i64::decode(binary(&[0xFF], signed_tiny.clone())).unwrap(),
        -1
    );
    assert!(u64::decode(binary(&[0xFF], signed_tiny)).is_err());

    let max = u64::MAX.to_le_bytes();
    let unsigned_big = column(ColumnType::LongLong, true);
    assert_eq!(
        u64::decode(binary(&max, unsigned_big.clone())).unwrap(),
        u64::MAX
    );
    assert!(i64::decode(binary(&max, unsigned_big)).is_err());

    let small = 40_000i64.to_le_bytes();
    let signed_big = column(ColumnType::LongLong, false);
    assert_eq!(
        i32::decode(binary(&small, signed_big.clone())).unwrap(),
        40_000
    );
    assert!(i16::decode(binary(&small, signed_big)).is_err());
}

#[test]
fn text_and_bit_values() {
    let value = MySqlValueRef {
        value: Some(b"18446744073709551615"),
        row: None,
        type_info: column(ColumnType::LongLong, true),
        format: MySqlValueFormat::Text,
    };
    assert_eq!(u64::decode(value).unwrap(), u64::MAX);

    // BIT is big-endian in either format.
    let bit = column(ColumnType::Bit, true);
    assert_eq!(
        i32::decode(binary(&[0x01, 0x00], bit.clone())).unwrap(),
        256
    );
    assert!(bool::decode(binary(&[0x80], bit)).unwrap());
}
