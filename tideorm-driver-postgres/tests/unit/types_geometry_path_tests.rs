use std::str::FromStr;

use crate::test_support::encoded;

use crate::types::PgPoint;

use super::PgPath;

const PATH_CLOSED_BYTES: &[u8] = &[
    1, 0, 0, 0, 2, 63, 240, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 64, 8, 0, 0, 0, 0, 0, 0, 64,
    16, 0, 0, 0, 0, 0, 0,
];

const PATH_OPEN_BYTES: &[u8] = &[
    0, 0, 0, 0, 2, 63, 240, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 64, 8, 0, 0, 0, 0, 0, 0, 64,
    16, 0, 0, 0, 0, 0, 0,
];

const PATH_UNEVEN_POINTS: &[u8] = &[
    0, 0, 0, 0, 2, 63, 240, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 64, 8, 0, 0, 0, 0, 0, 0, 64,
    16, 0, 0,
];

#[test]
fn can_deserialise_path_type_bytes_closed() {
    let path = PgPath::from_bytes(PATH_CLOSED_BYTES).unwrap();
    assert_eq!(
        path,
        PgPath {
            closed: true,
            points: vec![PgPoint { x: 1.0, y: 2.0 }, PgPoint { x: 3.0, y: 4.0 }]
        }
    )
}

#[test]
fn cannot_deserialise_path_type_uneven_point_bytes() {
    let path = PgPath::from_bytes(PATH_UNEVEN_POINTS);
    assert!(path.is_err());

    if let Err(err) = path {
        assert_eq!(err.to_string(), "expected 32 bytes after header, got 28")
    }
}

#[test]
fn can_deserialise_path_type_bytes_open() {
    let path = PgPath::from_bytes(PATH_OPEN_BYTES).unwrap();
    assert_eq!(
        path,
        PgPath {
            closed: false,
            points: vec![PgPoint { x: 1.0, y: 2.0 }, PgPoint { x: 3.0, y: 4.0 }]
        }
    )
}

#[test]
fn can_deserialise_path_type_str_first_syntax() {
    let path = PgPath::from_str("[( 1, 2), (3, 4 )]").unwrap();
    assert_eq!(
        path,
        PgPath {
            closed: false,
            points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }]
        }
    );
}

#[test]
fn cannot_deserialise_path_type_str_uneven_points_first_syntax() {
    let input_str = "[( 1, 2), (3)]";
    let path = PgPath::from_str(input_str);

    assert!(path.is_err());

    if let Err(err) = path {
        assert_eq!(
            err.to_string(),
            format!("error occurred while decoding: Unmatched pair in PATH: {input_str}")
        )
    }
}

#[test]
fn can_deserialise_path_type_str_second_syntax() {
    let path = PgPath::from_str("(( 1, 2), (3, 4 ))").unwrap();
    assert_eq!(
        path,
        PgPath {
            closed: true,
            points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }]
        }
    );
}

#[test]
fn can_deserialise_path_type_str_third_syntax() {
    let path = PgPath::from_str("(1, 2), (3, 4 )").unwrap();
    assert_eq!(
        path,
        PgPath {
            closed: true,
            points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }]
        }
    );
}

#[test]
fn can_deserialise_path_type_str_fourth_syntax() {
    let path = PgPath::from_str("1, 2, 3, 4").unwrap();
    assert_eq!(
        path,
        PgPath {
            closed: true,
            points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }]
        }
    );
}

#[test]
fn can_deserialise_path_type_str_float() {
    let path = PgPath::from_str("(1.1, 2.2), (3.3, 4.4)").unwrap();
    assert_eq!(
        path,
        PgPath {
            closed: true,
            points: vec![PgPoint { x: 1.1, y: 2.2 }, PgPoint { x: 3.3, y: 4.4 }]
        }
    );
}

#[test]
fn can_serialise_path_type() {
    let path = PgPath {
        closed: true,
        points: vec![PgPoint { x: 1., y: 2. }, PgPoint { x: 3., y: 4. }],
    };
    assert_eq!(encoded(&path), PATH_CLOSED_BYTES,)
}
