use std::str::FromStr;

use crate::test_support::encoded;

use super::PgCube;

const POINT_BYTES: &[u8] = &[128, 0, 0, 1, 64, 0, 0, 0, 0, 0, 0, 0];
const ZERO_VOLUME_BYTES: &[u8] = &[
    128, 0, 0, 2, 64, 0, 0, 0, 0, 0, 0, 0, 64, 8, 0, 0, 0, 0, 0, 0,
];
const ONE_DIMENSIONAL_INTERVAL_BYTES: &[u8] = &[
    0, 0, 0, 1, 64, 28, 0, 0, 0, 0, 0, 0, 64, 32, 0, 0, 0, 0, 0, 0,
];
const MULTI_DIMENSION_2_DIM_BYTES: &[u8] = &[
    0, 0, 0, 2, 63, 240, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 64, 8, 0, 0, 0, 0, 0, 0, 64,
    16, 0, 0, 0, 0, 0, 0,
];
const MULTI_DIMENSION_3_DIM_BYTES: &[u8] = &[
    0, 0, 0, 3, 64, 0, 0, 0, 0, 0, 0, 0, 64, 8, 0, 0, 0, 0, 0, 0, 64, 16, 0, 0, 0, 0, 0, 0, 64, 20,
    0, 0, 0, 0, 0, 0, 64, 24, 0, 0, 0, 0, 0, 0, 64, 28, 0, 0, 0, 0, 0, 0,
];

#[test]
fn can_deserialise_point_type_byes() {
    let cube = PgCube::from_bytes(POINT_BYTES).unwrap();
    assert_eq!(cube, PgCube::Point(2.))
}

#[test]
fn can_deserialise_point_type_str() {
    let cube_1 = PgCube::from_str("(2)").unwrap();
    assert_eq!(cube_1, PgCube::Point(2.));
    let cube_2 = PgCube::from_str("2").unwrap();
    assert_eq!(cube_2, PgCube::Point(2.));
}

#[test]
fn can_serialise_point_type() {
    assert_eq!(encoded(&PgCube::Point(2.)), POINT_BYTES,)
}
#[test]
fn can_deserialise_zero_volume_bytes() {
    let cube = PgCube::from_bytes(ZERO_VOLUME_BYTES).unwrap();
    assert_eq!(cube, PgCube::ZeroVolume(vec![2., 3.]));
}

#[test]
fn can_deserialise_zero_volume_string() {
    let cube_1 = PgCube::from_str("(2,3,4)").unwrap();
    assert_eq!(cube_1, PgCube::ZeroVolume(vec![2., 3., 4.]));
    let cube_2 = PgCube::from_str("2,3,4").unwrap();
    assert_eq!(cube_2, PgCube::ZeroVolume(vec![2., 3., 4.]));
}

#[test]
fn can_serialise_zero_volume() {
    assert_eq!(
        encoded(&PgCube::ZeroVolume(vec![2., 3.])),
        ZERO_VOLUME_BYTES
    );
}

#[test]
fn can_deserialise_one_dimension_interval_bytes() {
    let cube = PgCube::from_bytes(ONE_DIMENSIONAL_INTERVAL_BYTES).unwrap();
    assert_eq!(cube, PgCube::OneDimensionInterval(7., 8.))
}

#[test]
fn can_deserialise_one_dimension_interval_string() {
    let cube_1 = PgCube::from_str("((7),(8))").unwrap();
    assert_eq!(cube_1, PgCube::OneDimensionInterval(7., 8.));
    let cube_2 = PgCube::from_str("(7),(8)").unwrap();
    assert_eq!(cube_2, PgCube::OneDimensionInterval(7., 8.));
}

#[test]
fn can_serialise_one_dimension_interval() {
    assert_eq!(
        encoded(&PgCube::OneDimensionInterval(7., 8.)),
        ONE_DIMENSIONAL_INTERVAL_BYTES
    )
}

#[test]
fn can_deserialise_multi_dimension_2_dimension_byte() {
    let cube = PgCube::from_bytes(MULTI_DIMENSION_2_DIM_BYTES).unwrap();
    assert_eq!(
        cube,
        PgCube::MultiDimension(vec![vec![1., 2.], vec![3., 4.]])
    )
}

#[test]
fn can_deserialise_multi_dimension_2_dimension_string() {
    let cube_1 = PgCube::from_str("((1,2),(3,4))").unwrap();
    assert_eq!(
        cube_1,
        PgCube::MultiDimension(vec![vec![1., 2.], vec![3., 4.]])
    );
    let cube_2 = PgCube::from_str("((1, 2), (3, 4))").unwrap();
    assert_eq!(
        cube_2,
        PgCube::MultiDimension(vec![vec![1., 2.], vec![3., 4.]])
    );
    let cube_3 = PgCube::from_str("(1,2),(3,4)").unwrap();
    assert_eq!(
        cube_3,
        PgCube::MultiDimension(vec![vec![1., 2.], vec![3., 4.]])
    );
    let cube_4 = PgCube::from_str("(1, 2), (3, 4)").unwrap();
    assert_eq!(
        cube_4,
        PgCube::MultiDimension(vec![vec![1., 2.], vec![3., 4.]])
    )
}

#[test]
fn can_serialise_multi_dimension_2_dimension() {
    assert_eq!(
        encoded(&PgCube::MultiDimension(vec![vec![1., 2.], vec![3., 4.]])),
        MULTI_DIMENSION_2_DIM_BYTES
    )
}

#[test]
fn can_deserialise_multi_dimension_3_dimension_bytes() {
    let cube = PgCube::from_bytes(MULTI_DIMENSION_3_DIM_BYTES).unwrap();
    assert_eq!(
        cube,
        PgCube::MultiDimension(vec![vec![2., 3., 4.], vec![5., 6., 7.]])
    )
}

#[test]
fn can_deserialise_multi_dimension_3_dimension_string() {
    let cube = PgCube::from_str("((2,3,4),(5,6,7))").unwrap();
    assert_eq!(
        cube,
        PgCube::MultiDimension(vec![vec![2., 3., 4.], vec![5., 6., 7.]])
    );
    let cube_2 = PgCube::from_str("(2,3,4),(5,6,7)").unwrap();
    assert_eq!(
        cube_2,
        PgCube::MultiDimension(vec![vec![2., 3., 4.], vec![5., 6., 7.]])
    );
}

#[test]
fn can_serialise_multi_dimension_3_dimension() {
    assert_eq!(
        encoded(&PgCube::MultiDimension(vec![
            vec![2., 3., 4.],
            vec![5., 6., 7.]
        ])),
        MULTI_DIMENSION_3_DIM_BYTES
    )
}
