use super::*;
use crate::column::ColumnOrigin;
use crate::ext::ustr::UStr;
use crate::io::ProtocolDecode;
use crate::protocol::statement::BinaryRow;
use crate::protocol::text::{ColumnType, TextRow};
use crate::{MySqlColumn, MySqlTypeInfo};

fn columns_of(types: &[ColumnType]) -> Vec<MySqlColumn> {
    types
        .iter()
        .enumerate()
        .map(|(ordinal, &ty)| MySqlColumn {
            ordinal,
            name: UStr::new("c"),
            type_info: MySqlTypeInfo::binary(ty),
            origin: ColumnOrigin::Unknown,
        })
        .collect()
}

#[test]
fn a_truncated_text_row_is_refused() {
    let columns = columns_of(&[ColumnType::VarString]);
    for row in [&b""[..], b"\x03ab", b"\xfc\x01"] {
        assert!(
            TextRow::decode_with(Bytes::copy_from_slice(row), &columns).is_err(),
            "{row:?}"
        );
    }

    let row = TextRow::decode_with(Bytes::from_static(b"\xfb"), &columns).unwrap();
    assert_eq!(row.0.get(0), None);
}

#[test]
fn a_truncated_binary_row_is_refused() {
    let columns = columns_of(&[ColumnType::LongLong, ColumnType::Datetime]);
    // the header, the NULL bitmap, a BIGINT, then a DATETIME led by its length
    let row = b"\x00\x00\x01\x02\x03\x04\x05\x06\x07\x08\x04\xe8\x07\x01\x02";
    for len in 0..row.len() {
        let truncated = Bytes::copy_from_slice(&row[..len]);
        assert!(
            BinaryRow::decode_with(truncated, &columns).is_err(),
            "{len}"
        );
    }
    assert!(BinaryRow::decode_with(Bytes::from_static(b"\x01\x00"), &columns).is_err());

    let decoded = BinaryRow::decode_with(Bytes::from_static(row), &columns).unwrap();
    assert_eq!(decoded.0.get(1), Some(&[4, 0xe8, 7, 1, 2][..]));

    // a value in a column typed NULL
    let null = columns_of(&[ColumnType::Null]);
    assert!(BinaryRow::decode_with(Bytes::from_static(b"\x00\x00"), &null).is_err());
}
