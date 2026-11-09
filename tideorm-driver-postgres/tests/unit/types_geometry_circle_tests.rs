use std::str::FromStr;

use crate::test_support::encoded;

use super::PgCircle;

const CIRCLE_BYTES: &[u8] = &[
    63, 241, 153, 153, 153, 153, 153, 154, 64, 1, 153, 153, 153, 153, 153, 154, 64, 10, 102, 102,
    102, 102, 102, 102,
];

#[test]
fn can_deserialise_circle_type_bytes() {
    let circle = PgCircle::from_bytes(CIRCLE_BYTES).unwrap();
    assert_eq!(
        circle,
        PgCircle {
            x: 1.1,
            y: 2.2,
            radius: 3.3
        }
    )
}

#[test]
fn can_deserialise_circle_type_str() {
    let circle = PgCircle::from_str("<(1, 2), 3 >").unwrap();
    assert_eq!(
        circle,
        PgCircle {
            x: 1.0,
            y: 2.0,
            radius: 3.0
        }
    );
}

#[test]
fn can_deserialise_circle_type_str_second_syntax() {
    let circle = PgCircle::from_str("((1, 2), 3 )").unwrap();
    assert_eq!(
        circle,
        PgCircle {
            x: 1.0,
            y: 2.0,
            radius: 3.0
        }
    );
}

#[test]
fn can_deserialise_circle_type_str_third_syntax() {
    let circle = PgCircle::from_str("(1, 2), 3 ").unwrap();
    assert_eq!(
        circle,
        PgCircle {
            x: 1.0,
            y: 2.0,
            radius: 3.0
        }
    );
}

#[test]
fn can_deserialise_circle_type_str_fourth_syntax() {
    let circle = PgCircle::from_str("1, 2, 3 ").unwrap();
    assert_eq!(
        circle,
        PgCircle {
            x: 1.0,
            y: 2.0,
            radius: 3.0
        }
    );
}

#[test]
fn cannot_deserialise_circle_invalid_numbers() {
    let input_str = "1, 2, Three";
    let circle = PgCircle::from_str(input_str);
    assert!(circle.is_err());
    if let Err(err) = circle {
        assert_eq!(
            err.to_string(),
            format!("error decoding CIRCLE: could not get radius from {input_str}")
        )
    }
}

#[test]
fn cannot_deserialise_circle_negative_radius() {
    let input_str = "1, 2, -3";
    let circle = PgCircle::from_str(input_str);
    assert!(circle.is_err());
    if let Err(err) = circle {
        assert_eq!(
            err.to_string(),
            format!("error decoding CIRCLE: cannot have negative radius: {input_str}")
        )
    }
}

#[test]
fn can_deserialise_circle_type_str_float() {
    let circle = PgCircle::from_str("<(1.1, 2.2), 3.3>").unwrap();
    assert_eq!(
        circle,
        PgCircle {
            x: 1.1,
            y: 2.2,
            radius: 3.3
        }
    );
}

#[test]
fn can_serialise_circle_type() {
    let circle = PgCircle {
        x: 1.1,
        y: 2.2,
        radius: 3.3,
    };
    assert_eq!(encoded(&circle), CIRCLE_BYTES,)
}
