/// `bytes` when they are exactly `N` long.
pub(super) fn fixed<const N: usize>(bytes: &[u8]) -> Option<[u8; N]> {
    bytes.try_into().ok()
}

/// The next big-endian `i32` of `bytes`, which it moves past.
pub(super) fn take_i32(bytes: &mut &[u8]) -> Option<i32> {
    let (head, tail) = bytes.split_first_chunk::<4>()?;
    *bytes = tail;
    Some(i32::from_be_bytes(*head))
}

/// The next length-prefixed value of `bytes`, which it moves past; `None`
/// inside for a `NULL`.
pub(super) fn take_value<'a>(bytes: &mut &'a [u8]) -> Option<Option<&'a [u8]>> {
    let length = take_i32(bytes)?;
    if length == -1 {
        return Some(None);
    }
    let (value, tail) = bytes.split_at_checked(usize::try_from(length).ok()?)?;
    *bytes = tail;
    Some(Some(value))
}
