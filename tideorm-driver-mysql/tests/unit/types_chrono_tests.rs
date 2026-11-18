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

#[test]
fn a_year_past_9999_is_refused() {
    let mut buf = Vec::new();
    for year in [10_000, -1] {
        let date = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
        assert!(encode_date(&date, &mut buf).is_err(), "{year}");
    }
    let last = NaiveDate::from_ymd_opt(9999, 12, 31).unwrap();
    encode_date(&last, &mut buf).expect("9999-12-31 is a MySQL date");
    assert_eq!(buf, [0x0F, 0x27, 12, 31]);
}
