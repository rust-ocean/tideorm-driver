use super::*;

#[test]
fn bytes_mut_init_mut_zero_fills_the_spare_capacity() {
    // (capacity, bytes written first, the slice `init_mut` returns)
    let cases: [(usize, &[u8], &[u8]); 4] = [
        (
            8,
            &[0x12, 0x34, 0x56, 0x78],
            &[0x12, 0x34, 0x56, 0x78, 0, 0, 0, 0],
        ),
        (3, &[], &[0, 0, 0]),
        (2, &[7, 9], &[7, 9]),
        (0, &[], &[]),
    ];

    for (capacity, written, expected) in cases {
        let mut buf = BytesMut::with_capacity(capacity);
        buf.put_slice(written);

        assert_eq!(
            buf.init_mut(),
            expected,
            "capacity {capacity}, wrote {written:?}"
        );
        // The zeros are now part of the buffer, not just its spare room.
        assert_eq!(&buf[..], expected);
    }
}

#[test]
fn slice_init_mut_is_the_slice_itself() {
    let mut storage = [1u8, 2, 3];
    let mut slice = &mut storage[..];

    slice.init_mut()[0] = 9;
    assert_eq!(storage, [9, 2, 3]);
}
