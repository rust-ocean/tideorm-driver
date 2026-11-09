use std::str::FromStr;

use crate::test_support::encoded;

use super::PgLine;

const LINE_BYTES: &[u8] = &[
    63, 241, 153, 153, 153, 153, 153, 154, 64, 1, 153, 153, 153, 153, 153, 154, 64, 10, 102, 102,
    102, 102, 102, 102,
];

#[test]
fn can_deserialise_line_type_bytes() {
    let line = PgLine::from_bytes(LINE_BYTES).unwrap();
    assert_eq!(
        line,
        PgLine {
            a: 1.1,
            b: 2.2,
            c: 3.3
        }
    )
}

#[test]
fn can_deserialise_line_type_str() {
    let line = PgLine::from_str("{ 1, 2, 3 }").unwrap();
    assert_eq!(
        line,
        PgLine {
            a: 1.0,
            b: 2.0,
            c: 3.0
        }
    );
}

#[test]
fn cannot_deserialise_line_too_few_numbers() {
    let input_str = "{ 1, 2 }";
    let line = PgLine::from_str(input_str);
    assert!(line.is_err());
    if let Err(err) = line {
        assert_eq!(
            err.to_string(),
            format!("error decoding LINE: could not get c from {input_str}")
        )
    }
}

#[test]
fn cannot_deserialise_line_too_many_numbers() {
    let input_str = "{ 1, 2, 3, 4 }";
    let line = PgLine::from_str(input_str);
    assert!(line.is_err());
    if let Err(err) = line {
        assert_eq!(
            err.to_string(),
            format!("error decoding LINE: too many numbers inputted in {input_str}")
        )
    }
}

#[test]
fn cannot_deserialise_line_invalid_numbers() {
    let input_str = "{ 1, 2, three }";
    let line = PgLine::from_str(input_str);
    assert!(line.is_err());
    if let Err(err) = line {
        assert_eq!(
            err.to_string(),
            format!("error decoding LINE: could not get c from {input_str}")
        )
    }
}

#[test]
fn can_deserialise_line_type_str_float() {
    let line = PgLine::from_str("{1.1, 2.2, 3.3}").unwrap();
    assert_eq!(
        line,
        PgLine {
            a: 1.1,
            b: 2.2,
            c: 3.3
        }
    );
}

#[test]
fn can_serialise_line_type() {
    let line = PgLine {
        a: 1.1,
        b: 2.2,
        c: 3.3,
    };
    assert_eq!(encoded(&line), LINE_BYTES,)
}
