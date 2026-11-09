use super::*;

#[test]
fn each_command_is_its_byte_and_then_its_arguments() {
    let mut arguments = MySqlArguments::default();
    arguments.add(1_i32).unwrap();
    arguments.add(None::<i64>).unwrap();
    arguments.add(u64::MAX).unwrap();
    arguments.add("hi").unwrap();
    let no_arguments = MySqlArguments::default();

    #[rustfmt::skip]
    let cases: [(Command<'_>, Vec<u8>); 7] = [
        (Command::Quit, vec![0x01]),
        (Command::Ping, vec![0x0e]),
        (Command::Query("SELECT 1"), b"\x03SELECT 1".to_vec()),
        (Command::Prepare("SELECT ?"), b"\x16SELECT ?".to_vec()),
        (Command::CloseStatement(0x0102_0304), vec![0x19, 0x04, 0x03, 0x02, 0x01]),
        (
            Command::Execute { statement: 7, arguments: &no_arguments },
            vec![0x17, 7, 0, 0, 0, 0x00, 1, 0, 0, 0],
        ),
        (
            Command::Execute { statement: 7, arguments: &arguments },
            [
                // statement, no cursor, one iteration
                &[0x17, 7, 0, 0, 0, 0x00, 1, 0, 0, 0][..],
                // the second value is NULL; the types follow
                &[0b0010, 1],
                // INT, BIGINT, BIGINT UNSIGNED, VAR_STRING
                &[0x03, 0x00, 0x08, 0x00, 0x08, 0x80, 0xfd, 0x00],
                &[1, 0, 0, 0],
                &[0xff; 8],
                b"\x02hi",
            ]
            .concat(),
        ),
    ];

    for (command, expected) in cases {
        let mut buf = Vec::new();
        command
            .encode_with(&mut buf, Capabilities::empty())
            .unwrap();
        assert_eq!(buf, expected, "{command:?}");
    }
}
