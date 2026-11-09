use super::*;

fn value(bytes: &[u8], format: MySqlValueFormat) -> MySqlValueRef<'_> {
    MySqlValueRef {
        value: Some(bytes),
        row: None,
        type_info: MySqlTypeInfo::binary(ColumnType::Double),
        format,
    }
}

#[test]
fn floats_read_both_widths_and_text() {
    let float = 1.1_f32.to_le_bytes();
    let double = 1.1_f64.to_le_bytes();
    let binary = MySqlValueFormat::Binary;

    assert_eq!(f32::decode(value(&float, binary)).unwrap(), 1.1_f32);
    assert_eq!(
        f64::decode(value(&float, binary)).unwrap(),
        f64::from(1.1_f32)
    );
    assert_eq!(f64::decode(value(&double, binary)).unwrap(), 1.1_f64);
    assert_eq!(f32::decode(value(&double, binary)).unwrap(), 1.1_f32);

    let text = MySqlValueFormat::Text;
    assert_eq!(f32::decode(value(b"-2.5e3", text)).unwrap(), -2500.0_f32);
    assert_eq!(f64::decode(value(b"0.1", text)).unwrap(), 0.1_f64);
}

#[test]
fn floats_refuse_other_widths() {
    let decimal = b"\x041.25";
    for (error, expected) in [
        (
            f32::decode(value(decimal, MySqlValueFormat::Binary)).unwrap_err(),
            "expected a FLOAT as 4 or 8 bytes, got 5 bytes; \
             note that decoding DECIMAL as `f32` is not supported due to differing semantics",
        ),
        (
            f64::decode(value(decimal, MySqlValueFormat::Binary)).unwrap_err(),
            "expected a DOUBLE as 4 or 8 bytes, got 5 bytes; \
             note that decoding DECIMAL as `f64` is not supported due to differing semantics",
        ),
    ] {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn floats_take_float_and_double_columns_only() {
    for (column, compatible) in [
        (ColumnType::Float, true),
        (ColumnType::Double, true),
        (ColumnType::NewDecimal, false),
        (ColumnType::Long, false),
    ] {
        let info = MySqlTypeInfo::binary(column);
        assert_eq!(
            <f32 as Type<MySql>>::compatible(&info),
            compatible,
            "{column:?}"
        );
        assert_eq!(
            <f64 as Type<MySql>>::compatible(&info),
            compatible,
            "{column:?}"
        );
    }
}
