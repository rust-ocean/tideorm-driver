use super::*;
use crate::{MySqlTypeInfo, MySqlValueFormat};
use MySqlDecodedValue as V;

/// `utf8mb4_bin`, a text collation the `BINARY` flag marks too.
const UTF8MB4_BIN: Collation = Collation(46);

fn column(ty: ColumnType) -> MySqlTypeInfo {
    MySqlTypeInfo::binary(ty)
}

fn decode(
    type_info: MySqlTypeInfo,
    bytes: Option<&[u8]>,
    format: MySqlValueFormat,
) -> Result<MySqlDecodedValue, BoxDynError> {
    MySqlValueRef {
        value: bytes,
        row: None,
        type_info,
        format,
    }
    .decode_lossless()
}

fn binary(type_info: MySqlTypeInfo, bytes: &[u8]) -> MySqlDecodedValue {
    decode(type_info, Some(bytes), MySqlValueFormat::Binary).unwrap()
}

fn text(type_info: MySqlTypeInfo, text: &str) -> MySqlDecodedValue {
    decode(type_info, Some(text.as_bytes()), MySqlValueFormat::Text).unwrap()
}

fn json(text: &str) -> JsonValue {
    text.parse().unwrap()
}

#[test]
fn decimals_keep_every_digit_in_both_protocols() {
    let digits = "12345678901234567890123456789012345.123456789012345678901234567890";
    for format in [MySqlValueFormat::Binary, MySqlValueFormat::Text] {
        for ty in [ColumnType::Decimal, ColumnType::NewDecimal] {
            assert_eq!(
                decode(column(ty), Some(digits.as_bytes()), format).unwrap(),
                V::Decimal(digits.into())
            );
        }
    }
}

#[test]
fn integers_follow_the_columns_signedness() {
    let mut unsigned = column(ColumnType::LongLong);
    unsigned.flags |= ColumnFlags::UNSIGNED;
    assert_eq!(
        binary(unsigned, &u64::MAX.to_le_bytes()),
        V::Unsigned(u64::MAX)
    );
    assert_eq!(
        binary(column(ColumnType::LongLong), &(-1_i64).to_le_bytes()),
        V::Integer(-1)
    );
    assert_eq!(
        binary(column(ColumnType::Bit), &[0x01, 0x02]),
        V::Unsigned(258)
    );
    let mut boolean = column(ColumnType::Tiny);
    boolean.max_size = Some(1);
    assert_eq!(binary(boolean, &[1]), V::Bool(true));
    assert_eq!(text(column(ColumnType::Long), "-7"), V::Integer(-7));
}

#[test]
fn text_is_read_by_collation_never_by_its_contents() {
    let mut bin_text = column(ColumnType::VarString);
    bin_text.collation = UTF8MB4_BIN;
    assert_eq!(binary(bin_text, b"00123"), V::Text("00123".into()));
    let mut uuid_text = column(ColumnType::String);
    uuid_text.collation = Collation::UTF8MB4_GENERAL_CI;
    uuid_text.max_size = Some(16);
    let uuid = "67e55044-10b1-426f-9247-bb680e5fe0c8";
    assert_eq!(binary(uuid_text, uuid.as_bytes()), V::Text(uuid.into()));

    let id = Uuid::parse_str(uuid).unwrap();
    let mut binary_16 = column(ColumnType::String);
    binary_16.max_size = Some(16);
    assert_eq!(binary(binary_16, id.as_bytes()), V::Uuid(id));
    assert_eq!(
        binary(column(ColumnType::VarString), id.as_bytes()),
        V::Bytes(id.as_bytes().to_vec())
    );
}

#[test]
fn json_and_uuid_follow_the_declared_or_extended_type() {
    assert_eq!(
        binary(column(ColumnType::Json), br#"{"a":[1]}"#),
        V::Json(json(r#"{"a":[1]}"#))
    );
    let mut mariadb_json = column(ColumnType::LongBlob);
    mariadb_json.collation = UTF8MB4_BIN;
    mariadb_json.extended = ExtendedType::Json;
    assert_eq!(binary(mariadb_json, b"[2]"), V::Json(json("[2]")));
    let mut mariadb_uuid = column(ColumnType::String);
    mariadb_uuid.collation = Collation::UTF8MB4_GENERAL_CI;
    mariadb_uuid.extended = ExtendedType::Uuid;
    let uuid = "67e55044-10b1-426f-9247-bb680e5fe0c8";
    assert_eq!(
        binary(mariadb_uuid, uuid.as_bytes()),
        V::Uuid(Uuid::parse_str(uuid).unwrap())
    );
}

#[test]
fn times_keep_their_sign_and_zero_dates_are_null() {
    match text(column(ColumnType::Time), "-01:30:00") {
        V::Time(time) => {
            assert!(time.is_negative());
            assert_eq!((time.hours(), time.minutes()), (1, 30));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        text(column(ColumnType::Date), "2024-02-29"),
        V::Date(NaiveDate::from_ymd_opt(2024, 2, 29).unwrap())
    );
    assert_eq!(binary(column(ColumnType::Date), b"\0"), V::Null);
    assert_eq!(
        decode(
            column(ColumnType::VarString),
            None,
            MySqlValueFormat::Binary
        )
        .unwrap(),
        V::Null
    );
}

#[test]
fn geometry_is_refused() {
    assert!(decode(
        column(ColumnType::Geometry),
        Some(&[0; 25]),
        MySqlValueFormat::Binary
    )
    .is_err());
}
