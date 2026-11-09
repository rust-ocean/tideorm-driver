use std::str::FromStr;

use crate::test_support::encoded;

use super::PgBox;

const BOX_BYTES: &[u8] = &[
    64, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0,
    0, 0,
];

#[test]
fn can_deserialise_box_type_bytes_in_order() {
    let pg_box = PgBox::from_bytes(BOX_BYTES).unwrap();
    assert_eq!(
        pg_box,
        PgBox {
            upper_right_x: 2.,
            upper_right_y: 2.,
            lower_left_x: -2.,
            lower_left_y: -2.
        }
    )
}

#[test]
fn can_deserialise_box_type_str_first_syntax() {
    let pg_box = PgBox::from_str("[( 1, 2), (3, 4 )]").unwrap();
    assert_eq!(
        pg_box,
        PgBox {
            upper_right_x: 1.,
            upper_right_y: 2.,
            lower_left_x: 3.,
            lower_left_y: 4.
        }
    );
}
#[test]
fn can_deserialise_box_type_str_second_syntax() {
    let pg_box = PgBox::from_str("(( 1, 2), (3, 4 ))").unwrap();
    assert_eq!(
        pg_box,
        PgBox {
            upper_right_x: 1.,
            upper_right_y: 2.,
            lower_left_x: 3.,
            lower_left_y: 4.
        }
    );
}

#[test]
fn can_deserialise_box_type_str_third_syntax() {
    let pg_box = PgBox::from_str("(1, 2), (3, 4 )").unwrap();
    assert_eq!(
        pg_box,
        PgBox {
            upper_right_x: 1.,
            upper_right_y: 2.,
            lower_left_x: 3.,
            lower_left_y: 4.
        }
    );
}

#[test]
fn can_deserialise_box_type_str_fourth_syntax() {
    let pg_box = PgBox::from_str("1, 2, 3, 4").unwrap();
    assert_eq!(
        pg_box,
        PgBox {
            upper_right_x: 1.,
            upper_right_y: 2.,
            lower_left_x: 3.,
            lower_left_y: 4.
        }
    );
}

#[test]
fn cannot_deserialise_too_many_numbers() {
    let input_str = "1, 2, 3, 4, 5";
    let pg_box = PgBox::from_str(input_str);
    assert!(pg_box.is_err());
    if let Err(err) = pg_box {
        assert_eq!(
            err.to_string(),
            format!("error decoding BOX: too many numbers inputted in {input_str}")
        )
    }
}

#[test]
fn cannot_deserialise_too_few_numbers() {
    let input_str = "1, 2, 3 ";
    let pg_box = PgBox::from_str(input_str);
    assert!(pg_box.is_err());
    if let Err(err) = pg_box {
        assert_eq!(
            err.to_string(),
            format!("error decoding BOX: could not get lower_left_y from {input_str}")
        )
    }
}

#[test]
fn cannot_deserialise_invalid_numbers() {
    let input_str = "1, 2, 3, FOUR";
    let pg_box = PgBox::from_str(input_str);
    assert!(pg_box.is_err());
    if let Err(err) = pg_box {
        assert_eq!(
            err.to_string(),
            format!("error decoding BOX: could not get lower_left_y from {input_str}")
        )
    }
}

#[test]
fn can_deserialise_box_type_str_float() {
    let pg_box = PgBox::from_str("(1.1, 2.2), (3.3, 4.4)").unwrap();
    assert_eq!(
        pg_box,
        PgBox {
            upper_right_x: 1.1,
            upper_right_y: 2.2,
            lower_left_x: 3.3,
            lower_left_y: 4.4
        }
    );
}

#[test]
fn can_serialise_box_type_in_order() {
    let pg_box = PgBox {
        upper_right_x: 2.,
        lower_left_x: -2.,
        upper_right_y: -2.,
        lower_left_y: 2.,
    };
    assert_eq!(encoded(&pg_box), BOX_BYTES,)
}

#[test]
fn can_serialise_box_type_out_of_order() {
    let pg_box = PgBox {
        upper_right_x: -2.,
        lower_left_x: 2.,
        upper_right_y: 2.,
        lower_left_y: -2.,
    };
    assert_eq!(encoded(&pg_box), BOX_BYTES,)
}

#[test]
fn can_order_box() {
    let pg_box = PgBox {
        upper_right_x: -2.,
        lower_left_x: 2.,
        upper_right_y: 2.,
        lower_left_y: -2.,
    };
    let bytes = encoded(&pg_box);

    let pg_box = PgBox::from_bytes(&bytes).unwrap();
    assert_eq!(
        pg_box,
        PgBox {
            upper_right_x: 2.,
            upper_right_y: 2.,
            lower_left_x: -2.,
            lower_left_y: -2.
        }
    )
}
