use super::*;
use std::fmt::Debug;

/// Every variant sends its keyword and parses back from it in any case; anything else
/// is a configuration error naming the pragma.
fn check<T>(pragma: &str, as_str: fn(&T) -> &'static str, variants: &[(T, &str)])
where
    T: FromStr<Err = Error> + Debug + PartialEq,
{
    for (variant, keyword) in variants {
        assert_eq!(as_str(variant), *keyword);
        for spelling in [
            keyword.to_string(),
            keyword.to_ascii_lowercase(),
            keyword[..1].to_string() + &keyword[1..].to_ascii_lowercase(),
        ] {
            assert_eq!(
                &spelling.parse::<T>().unwrap(),
                variant,
                "{pragma} = {spelling}"
            );
        }
    }

    for unknown in ["", "wall", " WAL", "NONE;", "ÉXTRA"] {
        match unknown.parse::<T>() {
            Err(Error::Configuration(message)) => assert_eq!(
                message.to_string(),
                format!("unknown value {unknown:?} for `{pragma}`")
            ),
            other => panic!("{pragma} = {unknown:?} gave {other:?}"),
        }
    }
}

#[test]
fn pragma_keywords_round_trip() {
    use SqliteAutoVacuum as V;
    assert_eq!(V::default(), V::None);
    check(
        "auto_vacuum",
        V::as_str,
        &[
            (V::None, "NONE"),
            (V::Full, "FULL"),
            (V::Incremental, "INCREMENTAL"),
        ],
    );

    use SqliteJournalMode as J;
    assert_eq!(J::default(), J::Wal);
    check(
        "journal_mode",
        J::as_str,
        &[
            (J::Delete, "DELETE"),
            (J::Truncate, "TRUNCATE"),
            (J::Persist, "PERSIST"),
            (J::Memory, "MEMORY"),
            (J::Wal, "WAL"),
            (J::Off, "OFF"),
        ],
    );

    use SqliteLockingMode as L;
    assert_eq!(L::default(), L::Normal);
    check(
        "locking_mode",
        L::as_str,
        &[(L::Normal, "NORMAL"), (L::Exclusive, "EXCLUSIVE")],
    );

    use SqliteSynchronous as S;
    assert_eq!(S::default(), S::Full);
    check(
        "synchronous",
        S::as_str,
        &[
            (S::Off, "OFF"),
            (S::Normal, "NORMAL"),
            (S::Full, "FULL"),
            (S::Extra, "EXTRA"),
        ],
    );
}
