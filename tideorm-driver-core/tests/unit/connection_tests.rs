use super::*;

#[test]
fn it_parses_username_with_at_sign_correctly() {
    let url = Url::parse("postgres://user@hostname:password@hostname:5432/database").unwrap();

    assert_eq!("user@hostname", decode_url_part(url.username()).unwrap());
}

#[test]
fn it_parses_password_with_non_ascii_chars_correctly() {
    let url = Url::parse("mysql://username:p@ssw0rd@hostname:5432/database").unwrap();

    assert_eq!(
        "p@ssw0rd",
        decode_url_part(url.password().unwrap()).unwrap()
    );
}
