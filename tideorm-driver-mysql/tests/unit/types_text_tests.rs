use super::*;
use crate::collation::Collation;
use crate::MySqlValueFormat;

fn text(text: &str) -> MySqlValueRef<'_> {
    MySqlValueRef {
        value: Some(text.as_bytes()),
        row: None,
        type_info: <str as Type<MySql>>::type_info(),
        format: MySqlValueFormat::Binary,
    }
}

fn encoded<'q>(value: impl Encode<'q, MySql>) -> Vec<u8> {
    let mut buf = Vec::new();
    assert!(matches!(value.encode(&mut buf).unwrap(), IsNull::No));
    buf
}

#[test]
fn text_values_are_written_as_length_encoded_strings() {
    let cases: [(Vec<u8>, &[u8]); 5] = [
        (encoded(Text(42_i32)), b"\x0242"),
        (encoded(IpAddr::from([10, 0, 0, 1])), b"\x0810.0.0.1"),
        (encoded(Ipv4Addr::LOCALHOST), b"\x09127.0.0.1"),
        (encoded(Ipv6Addr::LOCALHOST), b"\x03::1"),
        (encoded(Decimal::new(-12345, 2)), b"\x07-123.45"),
    ];

    for (bytes, expected) in cases {
        assert_eq!(bytes, expected);
    }
}

#[test]
fn text_values_are_parsed_back_or_refused() {
    assert_eq!(Text::<i32>::decode(text("42")).unwrap().0, 42);
    assert_eq!(
        IpAddr::decode(text("::1")).unwrap(),
        IpAddr::from(Ipv6Addr::LOCALHOST)
    );
    assert_eq!(
        Ipv4Addr::decode(text("10.0.0.1")).unwrap(),
        Ipv4Addr::new(10, 0, 0, 1)
    );
    assert_eq!(Ipv6Addr::decode(text("::1")).unwrap(), Ipv6Addr::LOCALHOST);
    assert_eq!(
        Decimal::decode(text("-123.45")).unwrap(),
        Decimal::new(-12345, 2)
    );

    assert!(Text::<i32>::decode(text("4x")).is_err());
    assert!(Ipv4Addr::decode(text("::1")).is_err());
    assert!(Decimal::decode(text("1.2.3")).is_err());
}

#[test]
fn addresses_take_text_columns_and_decimals_take_decimal_columns() {
    let mut binary_text = <str as Type<MySql>>::type_info();
    binary_text.collation = Collation::BINARY;

    // (column, read as an address, read as a decimal)
    let cases = [
        (<str as Type<MySql>>::type_info(), true, false),
        (binary_text, false, false),
        (MySqlTypeInfo::binary(ColumnType::NewDecimal), false, true),
        (MySqlTypeInfo::binary(ColumnType::Decimal), false, true),
        (MySqlTypeInfo::binary(ColumnType::Double), false, false),
    ];

    for (column, address, decimal) in cases {
        assert_eq!(
            <IpAddr as Type<MySql>>::compatible(&column),
            address,
            "{column:?}"
        );
        assert_eq!(
            <Ipv6Addr as Type<MySql>>::compatible(&column),
            address,
            "{column:?}"
        );
        assert_eq!(
            <Text<i32> as Type<MySql>>::compatible(&column),
            address,
            "{column:?}"
        );
        assert_eq!(
            <Decimal as Type<MySql>>::compatible(&column),
            decimal,
            "{column:?}"
        );
    }
}
