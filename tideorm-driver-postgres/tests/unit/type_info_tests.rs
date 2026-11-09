use super::*;

/// Type names compare as SQL identifiers: unquoted parts ignore ASCII case, quoted parts match
/// exactly, and each part of a qualified name is compared on its own. The relation is symmetric
/// but not transitive (`foo` equals both `"foo"` and `FOO`, which differ).
#[test]
fn name_eq_compares_as_identifiers() {
    let cases = [
        // unquoted names ignore case
        ("foo", "foo", true),
        ("foo", "Foo", true),
        ("foo", "FOO", true),
        ("foo.foo", "foo.foo", true),
        ("foo.foo", "foo.Foo", true),
        ("foo.foo", "foo.FOO", true),
        ("foo.foo", "Foo.foo", true),
        ("foo.foo", "Foo.Foo", true),
        ("foo.foo", "FOO.FOO", true),
        // a quoted part keeps its case
        ("foo", r#""foo""#, true),
        ("foo", r#""Foo""#, false),
        ("foo.foo", r#"foo."foo""#, true),
        ("foo.foo", r#"foo."Foo""#, false),
        ("foo.foo", r#"foo."FOO""#, false),
        // a qualified name is not its last part
        ("foo", "foo.foo", false),
        ("foo.foo", "foo", false),
        // a name with Unicode escapes must be written the same way
        (r#"U&"d\0061t""#, r#"U&"d\0061t""#, true),
        (r#"U&"d\0061t""#, "dat", false),
    ];

    for (a, b, equal) in cases {
        assert_eq!(name_eq(a, b), equal, "name_eq({a:?}, {b:?})");
        assert_eq!(name_eq(b, a), equal, "name_eq({b:?}, {a:?})");
    }
}
