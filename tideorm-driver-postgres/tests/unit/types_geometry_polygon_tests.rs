use std::str::FromStr;

use crate::test_support::encoded;

use crate::types::PgPoint;

use super::PgPolygon;

const POLYGON_BYTES: &[u8] = &[
    0, 0, 0, 12, 192, 0, 0, 0, 0, 0, 0, 0, 192, 8, 0, 0, 0, 0, 0, 0, 191, 240, 0, 0, 0, 0, 0, 0,
    192, 8, 0, 0, 0, 0, 0, 0, 191, 240, 0, 0, 0, 0, 0, 0, 191, 240, 0, 0, 0, 0, 0, 0, 63, 240, 0,
    0, 0, 0, 0, 0, 63, 240, 0, 0, 0, 0, 0, 0, 63, 240, 0, 0, 0, 0, 0, 0, 64, 8, 0, 0, 0, 0, 0, 0,
    64, 0, 0, 0, 0, 0, 0, 0, 64, 8, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 192, 8, 0, 0, 0, 0,
    0, 0, 63, 240, 0, 0, 0, 0, 0, 0, 192, 8, 0, 0, 0, 0, 0, 0, 63, 240, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 191, 240, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 191, 240, 0, 0, 0, 0, 0, 0,
    192, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 0,
];

#[test]
fn can_deserialise_polygon_type_bytes() {
    let polygon = PgPolygon::from_bytes(POLYGON_BYTES).unwrap();
    assert_eq!(
        polygon,
        PgPolygon {
            points: vec![
                PgPoint { x: -2., y: -3. },
                PgPoint { x: -1., y: -3. },
                PgPoint { x: -1., y: -1. },
                PgPoint { x: 1., y: 1. },
                PgPoint { x: 1., y: 3. },
                PgPoint { x: 2., y: 3. },
                PgPoint { x: 2., y: -3. },
                PgPoint { x: 1., y: -3. },
                PgPoint { x: 1., y: 0. },
                PgPoint { x: -1., y: 0. },
                PgPoint { x: -1., y: -2. },
                PgPoint { x: -2., y: -2. }
            ]
        }
    )
}

#[test]
fn can_deserialise_polygon_type_str_first_syntax() {
    let polygon = PgPolygon::from_str("[( 1, 2), (3, 4 )]").unwrap();
    assert_eq!(
        polygon,
        PgPolygon {
            points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }]
        }
    );
}

#[test]
fn can_deserialise_polygon_type_str_second_syntax() {
    let polygon = PgPolygon::from_str("(( 1, 2), (3, 4 ))").unwrap();
    assert_eq!(
        polygon,
        PgPolygon {
            points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }]
        }
    );
}

#[test]
fn cannot_deserialise_polygon_type_str_uneven_points_first_syntax() {
    let input_str = "[( 1, 2), (3)]";
    let polygon = PgPolygon::from_str(input_str);

    assert!(polygon.is_err());

    if let Err(err) = polygon {
        assert_eq!(
            err.to_string(),
            format!("error occurred while decoding: Unmatched pair in POLYGON: {input_str}")
        )
    }
}

#[test]
fn cannot_deserialise_polygon_type_str_invalid_numbers() {
    let input_str = "[( 1, 2), (2, three)]";
    let polygon = PgPolygon::from_str(input_str);

    assert!(polygon.is_err());

    if let Err(err) = polygon {
        assert_eq!(
            err.to_string(),
            "error occurred while decoding: could not get y"
        )
    }
}

#[test]
fn can_deserialise_polygon_type_str_third_syntax() {
    let polygon = PgPolygon::from_str("(1, 2), (3, 4 )").unwrap();
    assert_eq!(
        polygon,
        PgPolygon {
            points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }]
        }
    );
}

#[test]
fn can_deserialise_polygon_type_str_fourth_syntax() {
    let polygon = PgPolygon::from_str("1, 2, 3, 4").unwrap();
    assert_eq!(
        polygon,
        PgPolygon {
            points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }]
        }
    );
}

#[test]
fn can_deserialise_polygon_type_str_float() {
    let polygon = PgPolygon::from_str("(1.1, 2.2), (3.3, 4.4)").unwrap();
    assert_eq!(
        polygon,
        PgPolygon {
            points: vec![PgPoint { x: 1.1, y: 2.2 }, PgPoint { x: 3.3, y: 4.4 }]
        }
    );
}

#[test]
fn can_serialise_polygon_type() {
    let polygon = PgPolygon {
        points: vec![
            PgPoint { x: -2., y: -3. },
            PgPoint { x: -1., y: -3. },
            PgPoint { x: -1., y: -1. },
            PgPoint { x: 1., y: 1. },
            PgPoint { x: 1., y: 3. },
            PgPoint { x: 2., y: 3. },
            PgPoint { x: 2., y: -3. },
            PgPoint { x: 1., y: -3. },
            PgPoint { x: 1., y: 0. },
            PgPoint { x: -1., y: 0. },
            PgPoint { x: -1., y: -2. },
            PgPoint { x: -2., y: -2. },
        ],
    };
    assert_eq!(encoded(&polygon), POLYGON_BYTES,)
}
