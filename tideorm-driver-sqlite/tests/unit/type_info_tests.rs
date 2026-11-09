use super::*;

/// Declared types read by SQLite's affinity rules, plus the driver's own date, time and
/// boolean names, in any case.
#[test]
fn declared_types_parse_by_affinity() {
    use DataType::*;

    let cases = [
        ("INT4", Int4),
        ("INT", Integer),
        ("INTEGER", Integer),
        ("INTBIG", Integer),
        ("MEDIUMINT", Integer),
        ("BIGINT", Integer),
        ("UNSIGNED BIG INT", Integer),
        ("INT8", Integer),
        // "INT" anywhere wins, even inside a float's name, as in SQLite.
        ("FLOATING POINT", Integer),
        ("CHARACTER(20)", Text),
        ("NCHAR(55)", Text),
        ("varchar(255)", Text),
        ("TEXT", Text),
        ("CLOB", Text),
        ("BLOB", Blob),
        ("REAL", Float),
        ("FLOAT", Float),
        ("DOUBLE PRECISION", Float),
        ("BOOLEAN", Bool),
        ("BOOL", Bool),
        ("DATETIME", Datetime),
        ("timestamp", Datetime),
        ("TIME", Time),
        ("DATE", Date),
    ];

    for (declared, expected) in cases {
        assert_eq!(
            declared.parse::<DataType>().unwrap(),
            expected,
            "{declared}"
        );
    }

    // NUMERIC affinity, and no declared type at all, have no type here.
    for declared in ["NUMERIC", "DECIMAL(10,5)", ""] {
        let error = declared.parse::<DataType>().unwrap_err();
        let lower = declared.to_ascii_lowercase();
        assert_eq!(error.to_string(), format!("unknown type: `{lower}`"));
    }
}

#[test]
fn each_type_has_its_sql_name() {
    use DataType::*;

    let cases = [
        (Null, "NULL"),
        (Text, "TEXT"),
        (Float, "REAL"),
        (Blob, "BLOB"),
        (Int4, "INTEGER"),
        (Integer, "INTEGER"),
        (Bool, "BOOLEAN"),
        (Date, "DATE"),
        (Time, "TIME"),
        (Datetime, "DATETIME"),
    ];

    for (ty, name) in cases {
        let info = SqliteTypeInfo(ty);
        assert_eq!(info.name(), name);
        assert_eq!(info.to_string(), name);
        assert_eq!(info.is_null(), ty == Null, "{name}");
    }
}
