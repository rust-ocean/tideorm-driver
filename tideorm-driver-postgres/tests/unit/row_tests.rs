use super::*;
use crate::message::BackendMessage;
use crate::PgTypeInfo;

/// A statement described before its table lost a column reads rows of fewer values than it has
/// columns: the missing value is an error, not a panic, and the others still read.
#[test]
fn a_value_the_row_lacks_is_an_error() {
    let column = |ordinal, name: &'static str| PgColumn {
        ordinal,
        name: name.into(),
        type_info: PgTypeInfo::INT4,
        origin: Default::default(),
        relation_id: None,
        relation_attribute_no: None,
    };
    let metadata = PgStatementMetadata {
        columns: vec![column(0, "a"), column(1, "b")],
        column_names: Arc::new([("a".into(), 0), ("b".into(), 1)].into()),
        ..Default::default()
    };
    let mut body = 1_u16.to_be_bytes().to_vec();
    body.extend(4_i32.to_be_bytes());
    body.extend(7_i32.to_be_bytes());
    let row = PgRow {
        data: DataRow::decode_body(body.into()).unwrap(),
        format: PgValueFormat::Binary,
        metadata: Arc::new(metadata),
    };

    assert_eq!(row.try_get::<i32, _>("a").unwrap(), 7);
    let error = row.try_get::<Option<i32>, _>("b").unwrap_err();
    assert!(
        matches!(error, Error::ColumnIndexOutOfBounds { len: 1, index: 1 }),
        "{error:?}"
    );
}
