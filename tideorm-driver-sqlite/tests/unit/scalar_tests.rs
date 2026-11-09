use super::*;
use crate::SqliteConnection;
use driver_core::connection::Connection;
use driver_core::query::query;
use driver_core::row::Row;
use std::any::type_name;

use DataType as D;

const EVERY_TYPE: [DataType; 10] = [
    D::Null,
    D::Integer,
    D::Float,
    D::Text,
    D::Blob,
    D::Bool,
    D::Int4,
    D::Date,
    D::Time,
    D::Datetime,
];

fn check_type<T: Type<Sqlite> + ?Sized>(reports: DataType, accepts: &[DataType]) {
    let name = type_name::<T>();
    assert_eq!(T::type_info().0, reports, "{name}");
    for ty in EVERY_TYPE {
        let compatible = T::compatible(&SqliteTypeInfo(ty));
        assert_eq!(compatible, accepts.contains(&ty), "{name} from {ty:?}");
    }
}

#[test]
fn each_type_reports_and_accepts_its_column_types() {
    check_type::<bool>(D::Bool, &[D::Bool, D::Int4, D::Integer]);
    check_type::<f32>(D::Float, &[D::Float]);
    check_type::<f64>(D::Float, &[D::Float]);
    check_type::<str>(D::Text, &[D::Text]);
    check_type::<&str>(D::Text, &[D::Text]);
    check_type::<String>(D::Text, &[D::Text]);
    check_type::<[u8]>(D::Blob, &[D::Blob, D::Text]);
    check_type::<&[u8]>(D::Blob, &[D::Blob, D::Text]);
    check_type::<Vec<u8>>(D::Blob, &[D::Blob, D::Text]);
    check_type::<Json<()>>(D::Text, &[D::Text]);
    check_type::<Text<i32>>(D::Text, &[D::Text]);
}

/// The argument one encoding pushes, as its `Debug` text.
fn pushed(
    encode: impl FnOnce(&mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError>,
) -> String {
    let mut buf = SqliteArgumentsBuffer::default();
    assert!(matches!(encode(&mut buf), Ok(IsNull::No)));
    let debug = format!("{buf:?}");
    debug["SqliteArgumentsBuffer([".len()..debug.len() - "])".len()].to_string()
}

fn by_ref<'q, T: Encode<'q, Sqlite>>(value: T) -> String {
    pushed(|buf| value.encode_by_ref(buf))
}

fn by_value<'q, T: Encode<'q, Sqlite>>(value: T) -> String {
    pushed(|buf| value.encode(buf))
}

#[test]
fn each_type_binds_as_its_argument() {
    const TEXT: &str = r#"Text("hi")"#;
    const SLICE: &str = r#"TextSlice("hi")"#;
    const BLOB: &str = "Blob([1, 2])";

    let cases = [
        (by_ref(true), "Int(1)"),
        (by_ref(false), "Int(0)"),
        (by_ref(1.5_f32), "Double(1.5)"),
        (by_ref(-2.25_f64), "Double(-2.25)"),
        (by_ref("hi"), TEXT),
        (by_ref(Box::<str>::from("hi")), TEXT),
        (by_value(Box::<str>::from("hi")), SLICE),
        (by_ref(String::from("hi")), TEXT),
        (by_value(String::from("hi")), TEXT),
        (by_ref(Cow::Borrowed("hi")), TEXT),
        (by_value(Cow::<str>::Owned("hi".into())), TEXT),
        (by_ref(Arc::<str>::from("hi")), TEXT),
        (by_value(Arc::<str>::from("hi")), SLICE),
        (by_value(Rc::<str>::from("hi")), TEXT),
        (by_ref(&[1_u8, 2][..]), BLOB),
        (by_ref(Box::<[u8]>::from([1, 2])), BLOB),
        (by_value(Box::<[u8]>::from([1, 2])), BLOB),
        (by_ref(vec![1_u8, 2]), BLOB),
        (by_value(vec![1_u8, 2]), BLOB),
        (by_ref(Cow::Borrowed(&[1_u8, 2][..])), BLOB),
        (by_value(Cow::<[u8]>::Owned(vec![1, 2])), BLOB),
        (by_value(Arc::<[u8]>::from([1, 2])), BLOB),
        (by_value(Rc::<[u8]>::from([1, 2])), BLOB),
        (by_ref(Json(vec![1, 2])), r#"Text("[1,2]")"#),
        (by_ref(Text(42)), r#"Text("42")"#),
    ];

    for (i, (pushed, expected)) in cases.iter().enumerate() {
        assert_eq!(pushed, expected, "case {i}");
    }
}

#[test]
fn values_read_back_as_they_were_bound() {
    driver_core::rt::test_block_on(async {
        let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        let row = query::<Sqlite>("SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?")
            .bind(true)
            .bind(false)
            .bind(1.5_f32)
            .bind(-2.25_f64)
            .bind("hi")
            .bind("hi")
            .bind(vec![0_u8, 255])
            .bind(vec![0_u8, 255])
            .bind(Json(vec![1, 2]))
            .bind(Text(42))
            .fetch_one(&mut conn)
            .await
            .unwrap();

        assert!(row.get::<bool, _>(0));
        assert!(!row.get::<bool, _>(1));
        assert_eq!(row.get::<f32, _>(2), 1.5);
        assert_eq!(row.get::<f64, _>(3), -2.25);
        assert_eq!(row.get::<&str, _>(4), "hi");
        assert_eq!(row.get::<String, _>(5), "hi");
        assert_eq!(row.get::<&[u8], _>(6), [0, 255]);
        assert_eq!(row.get::<Vec<u8>, _>(7), [0, 255]);
        assert_eq!(row.get::<Json<Vec<i32>>, _>(8).0, [1, 2]);
        assert_eq!(row.get::<Text<i32>, _>(9).0, 42);
    });
}
