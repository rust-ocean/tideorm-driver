use crate::io::PortalId;
use crate::message::FrontendMessage;

use super::Execute;

#[test]
fn execute_encodes_portal_name_and_row_limit() {
    // 'E', the length (9), the portal's NUL-terminated name (empty: the unnamed portal), and the
    // row limit, where 0 means no limit
    let cases: [(u32, &[u8]); 2] = [
        (1_234_567_890, b"E\0\0\0\x09\0\x49\x96\x02\xD2"),
        (0, b"E\0\0\0\x09\0\0\0\0\0"),
    ];

    for (limit, expected) in cases {
        let mut buf = Vec::new();
        Execute {
            portal: PortalId::UNNAMED,
            limit,
        }
        .encode_msg(&mut buf)
        .unwrap();
        assert_eq!(buf, expected, "limit {limit}");
    }
}
