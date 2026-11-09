use super::*;

#[test]
fn command_tags_report_the_rows_they_affected() {
    for (tag, rows) in [
        (&b"INSERT 0 1214\0"[..], 1214),
        (b"BEGIN\0", 0),
        (b"UPDATE 5\0", 5),
    ] {
        let cc = CommandComplete::decode_body(Bytes::from_static(tag)).unwrap();

        assert_eq!(cc.rows_affected(), rows, "{tag:?}");
    }
}
