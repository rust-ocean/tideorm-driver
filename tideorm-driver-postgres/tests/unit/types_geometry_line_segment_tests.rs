use std::str::FromStr;

use crate::test_support::encoded;

use super::PgLSeg;

const LINE_SEGMENT_BYTES: &[u8] = &[
    63, 241, 153, 153, 153, 153, 153, 154, 64, 1, 153, 153, 153, 153, 153, 154, 64, 10, 102, 102,
    102, 102, 102, 102, 64, 17, 153, 153, 153, 153, 153, 154,
];

#[test]
fn can_deserialise_lseg_type_bytes() {
    let lseg = PgLSeg::from_bytes(LINE_SEGMENT_BYTES).unwrap();
    assert_eq!(
        lseg,
        PgLSeg {
            start_x: 1.1,
            start_y: 2.2,
            end_x: 3.3,
            end_y: 4.4
        }
    )
}

#[test]
fn can_deserialise_lseg_type_str_first_syntax() {
    let lseg = PgLSeg::from_str("[( 1, 2), (3, 4 )]").unwrap();
    assert_eq!(
        lseg,
        PgLSeg {
            start_x: 1.,
            start_y: 2.,
            end_x: 3.,
            end_y: 4.
        }
    );
}
#[test]
fn can_deserialise_lseg_type_str_second_syntax() {
    let lseg = PgLSeg::from_str("(( 1, 2), (3, 4 ))").unwrap();
    assert_eq!(
        lseg,
        PgLSeg {
            start_x: 1.,
            start_y: 2.,
            end_x: 3.,
            end_y: 4.
        }
    );
}

#[test]
fn can_deserialise_lseg_type_str_third_syntax() {
    let lseg = PgLSeg::from_str("(1, 2), (3, 4 )").unwrap();
    assert_eq!(
        lseg,
        PgLSeg {
            start_x: 1.,
            start_y: 2.,
            end_x: 3.,
            end_y: 4.
        }
    );
}

#[test]
fn can_deserialise_lseg_type_str_fourth_syntax() {
    let lseg = PgLSeg::from_str("1, 2, 3, 4").unwrap();
    assert_eq!(
        lseg,
        PgLSeg {
            start_x: 1.,
            start_y: 2.,
            end_x: 3.,
            end_y: 4.
        }
    );
}

#[test]
fn can_deserialise_too_many_numbers() {
    let input_str = "1, 2, 3, 4, 5";
    let lseg = PgLSeg::from_str(input_str);
    assert!(lseg.is_err());
    if let Err(err) = lseg {
        assert_eq!(
            err.to_string(),
            format!("error decoding LSEG: too many numbers inputted in {input_str}")
        )
    }
}

#[test]
fn can_deserialise_too_few_numbers() {
    let input_str = "1, 2, 3";
    let lseg = PgLSeg::from_str(input_str);
    assert!(lseg.is_err());
    if let Err(err) = lseg {
        assert_eq!(
            err.to_string(),
            format!("error decoding LSEG: could not get end_y from {input_str}")
        )
    }
}

#[test]
fn can_deserialise_invalid_numbers() {
    let input_str = "1, 2, 3, FOUR";
    let lseg = PgLSeg::from_str(input_str);
    assert!(lseg.is_err());
    if let Err(err) = lseg {
        assert_eq!(
            err.to_string(),
            format!("error decoding LSEG: could not get end_y from {input_str}")
        )
    }
}

#[test]
fn can_deserialise_lseg_type_str_float() {
    let lseg = PgLSeg::from_str("(1.1, 2.2), (3.3, 4.4)").unwrap();
    assert_eq!(
        lseg,
        PgLSeg {
            start_x: 1.1,
            start_y: 2.2,
            end_x: 3.3,
            end_y: 4.4
        }
    );
}

#[test]
fn can_serialise_lseg_type() {
    let lseg = PgLSeg {
        start_x: 1.1,
        start_y: 2.2,
        end_x: 3.3,
        end_y: 4.4,
    };
    assert_eq!(encoded(&lseg), LINE_SEGMENT_BYTES,)
}
