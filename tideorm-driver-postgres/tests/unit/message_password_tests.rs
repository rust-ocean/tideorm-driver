use crate::message::FrontendMessage;

use super::Password;

#[test]
fn password_message_carries_the_secret() {
    // the MD5 form is "md5" + md5(md5(password + username) as hex + salt) as hex
    let cases: [(Password<'_>, &[u8]); 2] = [
        (Password::Cleartext("password"), b"p\0\0\0\rpassword\0"),
        (
            Password::Md5 {
                password: "password",
                username: "root",
                salt: [147, 24, 57, 152],
            },
            b"p\0\0\0(md53e2c9d99d49b201ef867a36f3f9ed62c\0",
        ),
    ];

    for (message, expected) in cases {
        let mut buf = Vec::new();
        message.encode_msg(&mut buf).unwrap();
        assert_eq!(buf, expected);
    }
}
