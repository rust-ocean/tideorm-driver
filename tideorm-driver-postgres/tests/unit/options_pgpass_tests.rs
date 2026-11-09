use super::*;
use std::borrow::Cow;

#[test]
fn test_find_next_field() {
    fn test_case<'a>(
        mut input: &'a str,
        result: Result<Cow<'a, str>, PGPassLineParseError>,
        rest: &str,
    ) {
        assert_eq!(find_next_field(&mut input), result);
        assert_eq!(input, rest);
    }

    // normal field
    test_case("foo:bar:baz", Ok(Cow::Borrowed("foo")), "bar:baz");
    // \ escaped
    test_case(
        "foo\\\\:bar:baz",
        Ok(Cow::Owned("foo\\".to_owned())),
        "bar:baz",
    );
    // : escaped
    test_case(
        "foo\\::bar:baz",
        Ok(Cow::Owned("foo:".to_owned())),
        "bar:baz",
    );
    // unnecessary escape
    test_case(
        "foo\\a:bar:baz",
        Ok(Cow::Owned("fooa".to_owned())),
        "bar:baz",
    );
    // other text after escape
    test_case(
        "foo\\\\a:bar:baz",
        Ok(Cow::Owned("foo\\a".to_owned())),
        "bar:baz",
    );
    // double escape
    test_case(
        "foo\\\\\\\\a:bar:baz",
        Ok(Cow::Owned("foo\\\\a".to_owned())),
        "bar:baz",
    );
    // utf8 support
    test_case("🦀:bar:baz", Ok(Cow::Borrowed("🦀")), "bar:baz");

    // missing delimiter (eof)
    test_case("foo", Err(PGPassLineParseError::UnexpectedEOL), "foo");
    // missing delimiter after escape
    test_case("foo\\:", Err(PGPassLineParseError::UnexpectedEOL), "foo\\:");
    // missing delimiter after unused trailing escape
    test_case("foo\\", Err(PGPassLineParseError::UnexpectedEOL), "foo\\");
}

#[test]
fn test_load_password_from_line() {
    // normal
    assert_eq!(
        load_password_from_line(
            "localhost:5432:bar:foo:baz",
            "localhost",
            5432,
            "foo",
            Some("bar"),
        ),
        Ok(Some("baz".to_owned()))
    );
    // wildcard
    assert_eq!(
        load_password_from_line("*:5432:bar:foo:baz", "localhost", 5432, "foo", Some("bar")),
        Ok(Some("baz".to_owned()))
    );
    // accept wildcard with missing db
    assert_eq!(
        load_password_from_line("localhost:5432:*:foo:baz", "localhost", 5432, "foo", None),
        Ok(Some("baz".to_owned()))
    );

    // doesn't match
    assert_eq!(
        load_password_from_line(
            "thishost:5432:bar:foo:baz",
            "thathost",
            5432,
            "foo",
            Some("bar")
        ),
        Ok(None)
    );
    // malformed entry
    assert_eq!(
        load_password_from_line(
            "localhost:5432:bar:foo",
            "localhost",
            5432,
            "foo",
            Some("bar")
        ),
        Err(PGPassLineParseError::UnexpectedEOL)
    );
    // Password with trailing whitespace
    assert_eq!(
        load_password_from_line("*:*:*:*:baz ", "localhost", 5432, "foo", Some("bar")),
        Ok(Some("baz ".to_owned()))
    );
    // Password with escaped colon
    assert_eq!(
        load_password_from_line("*:*:*:*:ba\\:z", "localhost", 5432, "foo", Some("bar")),
        Ok(Some("ba:z".to_owned()))
    );
    // Password with escaped backslash
    assert_eq!(
        load_password_from_line("*:*:*:*:ba\\\\z", "localhost", 5432, "foo", Some("bar")),
        Ok(Some("ba\\z".to_owned()))
    );
    // Password with superfluous escape
    assert_eq!(
        load_password_from_line("*:*:*:*:ba\\z", "localhost", 5432, "foo", Some("bar")),
        Ok(Some("baz".to_owned()))
    );
    // Password with trailing escape
    assert_eq!(
        load_password_from_line("*:*:*:*:baz\\", "localhost", 5432, "foo", Some("bar")),
        Ok(Some("baz".to_owned()))
    );
}

#[test]
fn test_load_password_from_reader() {
    let file = b"\
        localhost:5432:bar:foo:baz\n\
        # mixed line endings (also a comment!)\n\
        *:5432:bar:foo:baz\r\n\
        # trailing space, comment with CRLF! \r\n\
        thishost:5432:bar:foo:baz \n\
        # malformed line \n\
        thathost:5432:foobar:foo\n\
        # missing trailing newline\n\
        localhost:5432:*:foo:baz
    ";

    // normal
    assert_eq!(
        load_password_from_reader(&mut &file[..], "localhost", 5432, "foo", Some("bar")),
        Some("baz".to_owned())
    );
    // wildcard
    assert_eq!(
        load_password_from_reader(&mut &file[..], "localhost", 5432, "foo", Some("foobar")),
        Some("baz".to_owned())
    );
    // accept wildcard with missing db
    assert_eq!(
        load_password_from_reader(&mut &file[..], "localhost", 5432, "foo", None),
        Some("baz".to_owned())
    );

    // doesn't match
    assert_eq!(
        load_password_from_reader(&mut &file[..], "thathost", 5432, "foo", Some("other")),
        None
    );
    // malformed entry
    assert_eq!(
        load_password_from_reader(&mut &file[..], "thathost", 5432, "foo", Some("foobar")),
        None
    );
}
