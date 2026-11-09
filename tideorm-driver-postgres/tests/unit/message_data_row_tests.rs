use super::*;

#[test]
fn test_decode_data_row() {
    const DATA: &[u8] = b"\
        \x00\x08\
        \xff\xff\xff\xff\
        \x00\x00\x00\x04\
        \x00\x00\x00\n\
        \xff\xff\xff\xff\
        \x00\x00\x00\x04\
        \x00\x00\x00\x14\
        \xff\xff\xff\xff\
        \x00\x00\x00\x04\
        \x00\x00\x00(\
        \xff\xff\xff\xff\
        \x00\x00\x00\x04\
        \x00\x00\x00P";

    let row = DataRow::decode_body(DATA.into()).unwrap();

    assert_eq!(row.values.len(), 8);

    assert!(row.get(0).is_none());
    assert_eq!(row.get(1).unwrap(), &[0_u8, 0, 0, 10][..]);
    assert!(row.get(2).is_none());
    assert_eq!(row.get(3).unwrap(), &[0_u8, 0, 0, 20][..]);
    assert!(row.get(4).is_none());
    assert_eq!(row.get(5).unwrap(), &[0_u8, 0, 0, 40][..]);
    assert!(row.get(6).is_none());
    assert_eq!(row.get(7).unwrap(), &[0_u8, 0, 0, 80][..]);
}

#[test]
fn a_value_ending_past_the_row_is_refused() {
    // one column of four bytes, two of them sent
    assert!(DataRow::decode_body(Bytes::from_static(b"\x00\x01\x00\x00\x00\x04ab")).is_err());
}

#[test]
fn only_a_length_of_minus_one_is_null() {
    assert!(DataRow::decode_body(Bytes::from_static(b"\x00\x01\xff\xff\xff\xfe")).is_err());

    let row = DataRow::decode_body(Bytes::from_static(b"\x00\x01\xff\xff\xff\xff")).unwrap();
    assert!(row.get(0).is_none());
}
