use super::wire::{take_i32, take_value};
use super::{PgDecodedArray, PgDecodedValue};

/// An array: its dimension count, a has-`NULL` flag, the element type, each
/// dimension's length and lower bound, then the elements in row-major order.
/// A dimension past the first nests one level deeper. The flag, the element
/// type and the lower bounds are read past, not kept.
pub(super) fn array_value(
    bytes: &[u8],
    element: &dyn Fn(&[u8]) -> Option<PgDecodedValue>,
) -> Option<PgDecodedValue> {
    let mut rest = bytes;
    let dimensions = take_i32(&mut rest)?;
    if !(0..=6).contains(&dimensions) {
        return None;
    }
    take_i32(&mut rest)?;
    take_i32(&mut rest)?;
    let mut lengths = Vec::with_capacity(dimensions.unsigned_abs() as usize);
    for _ in 0..dimensions {
        lengths.push(usize::try_from(take_i32(&mut rest)?).ok()?);
        take_i32(&mut rest)?;
    }
    if lengths.is_empty() {
        return rest.is_empty().then(|| {
            PgDecodedValue::Array(PgDecodedArray {
                dimensions: lengths,
                elements: Vec::new(),
            })
        });
    }
    let count = lengths
        .iter()
        .try_fold(1_usize, |count, length| count.checked_mul(*length))?;
    if count > rest.len() / 4 {
        return None;
    }
    let mut elements = Vec::with_capacity(count);
    for _ in 0..count {
        elements.push(match take_value(&mut rest)? {
            Some(value) => element(value)?,
            None => PgDecodedValue::Null,
        });
    }
    if !rest.is_empty() {
        return None;
    }
    Some(PgDecodedValue::Array(PgDecodedArray {
        dimensions: lengths,
        elements,
    }))
}
