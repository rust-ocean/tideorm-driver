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
