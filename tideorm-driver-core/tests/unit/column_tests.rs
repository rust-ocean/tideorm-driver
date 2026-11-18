use super::*;

/// A name one column has finds it; a name two columns share is an error, never either column.
#[test]
fn a_name_two_columns_share_finds_neither() {
    let mut names = HashMap::new();
    for (ordinal, name) in ["id", "name", "id", "Name"].into_iter().enumerate() {
        insert_column_name(&mut names, UStr::Static(name), ordinal);
    }
    assert_eq!(column_by_name(&names, "name").unwrap(), 1);
    assert_eq!(column_by_name(&names, "Name").unwrap(), 3);
    assert!(matches!(
        column_by_name(&names, "id"),
        Err(Error::ColumnDecode { index, .. }) if index == "id"
    ));
    assert!(matches!(
        column_by_name(&names, "ID"),
        Err(Error::ColumnNotFound(name)) if name == "ID"
    ));
}
