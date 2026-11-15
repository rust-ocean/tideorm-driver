use super::*;

#[test]
fn null_bitmap_sets_one_bit_per_null_from_the_lowest() {
    // (which values are NULL, the bitmap)
    let cases: [(&[bool], &[u8]); 5] = [
        (&[], &[]),
        (&[false], &[0]),
        (&[true], &[0b1]),
        (
            &[true, false, true, false, true, false, true, false, true],
            &[0b0101_0101, 0b1],
        ),
        (&[false; 16], &[0, 0]),
    ];

    for (nulls, expected) in cases {
        let mut bitmap = NullBitMap::default();
        for &null in nulls {
            bitmap.push(if null { IsNull::Yes } else { IsNull::No });
        }
        assert_eq!(&*bitmap, expected, "{nulls:?}");
    }
}

#[test]
fn a_value_that_fails_to_encode_leaves_the_arguments_as_they_were() {
    struct Unencodable;

    impl Type<MySql> for Unencodable {
        fn type_info() -> MySqlTypeInfo {
            <i32 as Type<MySql>>::type_info()
        }
    }

    impl Encode<'_, MySql> for Unencodable {
        fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
            buf.extend_from_slice(b"half");
            Err("cannot encode".into())
        }
    }

    let mut arguments = MySqlArguments::default();
    arguments.add(7_i32).unwrap();
    arguments.add(Unencodable).unwrap_err();

    assert_eq!(arguments.values, 7_i32.to_le_bytes());
    assert_eq!(arguments.types.len(), 1);
    assert_eq!(&*arguments.null_bitmap, &[0]);
}

#[test]
fn unsigned_integers_with_their_top_bit_set_are_sent_signed_for_vitess() {
    let mut arguments = MySqlArguments::default();
    arguments.add(u8::MAX).unwrap();
    arguments.add(Option::<u16>::None).unwrap();
    arguments.add("text").unwrap();
    arguments.add(i64::MAX as u64).unwrap();
    arguments.add(u64::MAX).unwrap();
    arguments.add(-1_i8).unwrap();

    let signed = arguments.signed().expect("some arguments are unsigned");
    let types: Vec<_> = signed
        .types
        .iter()
        .map(|ty| (ty.r#type, ty.flags.contains(ColumnFlags::UNSIGNED)))
        .collect();
    assert_eq!(
        types,
        [
            (ColumnType::LongLong, false),
            (ColumnType::Short, true),
            (ColumnType::VarString, false),
            (ColumnType::LongLong, true),
            (ColumnType::NewDecimal, false),
            (ColumnType::Tiny, false),
        ]
    );
    let mut values = 255_i64.to_le_bytes().to_vec();
    values.extend_from_slice(b"\x04text");
    values.extend_from_slice(&i64::MAX.to_le_bytes());
    values.extend_from_slice(b"\x1418446744073709551615");
    values.push(0xff);
    assert_eq!(signed.values, values);
    assert_eq!(&*signed.null_bitmap, &*arguments.null_bitmap);

    // A value with its top bit clear reads the same signed.
    let mut plain = MySqlArguments::default();
    plain.add(7_i32).unwrap();
    plain.add(true).unwrap();
    plain.add(u32::MAX >> 1).unwrap();
    assert!(plain.signed().is_none(), "nothing to rewrite");
}
