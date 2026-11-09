use super::*;

#[test]
fn null_and_err_markers_begin_no_integer() {
    for marker in [0xfb, 0xff] {
        assert!(Bytes::copy_from_slice(&[marker]).get_uint_lenenc().is_err());
    }

    assert_eq!(Bytes::from_static(&[0xfa]).get_uint_lenenc().unwrap(), 0xfa);
    assert_eq!(
        Bytes::from_static(&[0xfc, 1, 2]).get_uint_lenenc().unwrap(),
        0x0201
    );
}

#[test]
fn bytes_running_past_the_buffer_are_refused() {
    for buf in [
        &[3, b'a', b'b'][..],
        &[0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    ] {
        assert!(
            Bytes::copy_from_slice(buf).get_bytes_lenenc().is_err(),
            "{buf:?}"
        );
    }

    let mut buf = Bytes::from_static(&[2, b'a', b'b', b'c']);
    assert_eq!(&buf.get_bytes_lenenc().unwrap()[..], b"ab");
    assert_eq!(&buf[..], b"c");
}
