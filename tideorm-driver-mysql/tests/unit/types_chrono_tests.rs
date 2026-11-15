use super::*;

#[test]
fn a_timestamp_column_reads_as_a_naive_or_zoned_date_time() {
    let stamp = [7, 0xEA, 0x07, 11, 14, 9, 30, 15];
    for ty in [ColumnType::Datetime, ColumnType::Timestamp] {
        let info = MySqlTypeInfo::binary(ty);
        assert!(<NaiveDateTime as Type<MySql>>::compatible(&info), "{ty:?}");
        assert!(<DateTime<Utc> as Type<MySql>>::compatible(&info), "{ty:?}");
        let value = MySqlValueRef {
            value: Some(&stamp),
            row: None,
            type_info: info,
            format: MySqlValueFormat::Binary,
        };
        let read: NaiveDateTime = Decode::<MySql>::decode(value).expect("decode failed");
        assert_eq!(read.to_string(), "2026-11-14 09:30:15");
    }
    let text = MySqlTypeInfo::binary(ColumnType::VarString);
    assert!(!<NaiveDateTime as Type<MySql>>::compatible(&text));
}
