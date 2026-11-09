use super::*;

#[test]
fn query_encodes_its_sql_nul_terminated() {
    let cases: [(&str, &[u8]); 2] = [
        ("SELECT 1", b"Q\0\0\0\x0DSELECT 1\0"),
        ("", b"Q\0\0\0\x05\0"),
    ];

    for (sql, expected) in cases {
        let mut buf = Vec::new();
        Query(sql).encode_msg(&mut buf).unwrap();
        assert_eq!(buf, expected, "{sql:?}");
    }
}
