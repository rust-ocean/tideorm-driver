use super::wire::{take_i32, take_value};
use crate::types::range::RangeFlags;

/// A range: its flags, then each bound it has, length-prefixed, written by
/// `bound`; as PostgreSQL prints it, `empty` or `[lower,upper)`.
pub(super) fn range_text(bytes: &[u8], bound: impl Fn(&[u8]) -> Option<String>) -> Option<String> {
    let (&flags, mut rest) = bytes.split_first()?;
    let flags = RangeFlags::from_bits_truncate(flags);
    if flags.contains(RangeFlags::EMPTY) {
        return rest.is_empty().then(|| "empty".to_string());
    }
    let mut text = String::from(if flags.contains(RangeFlags::LB_INC) {
        '['
    } else {
        '('
    });
    for (unbounded, separator) in [(RangeFlags::LB_INF, Some(',')), (RangeFlags::UB_INF, None)] {
        if !flags.contains(unbounded) {
            push_range_bound(&mut text, &bound(take_value(&mut rest)??)?);
        }
        text.extend(separator);
    }
    text.push(if flags.contains(RangeFlags::UB_INC) {
        ']'
    } else {
        ')'
    });
    rest.is_empty().then_some(text)
}

/// A range bound, double-quoted when it is empty or holds a character the
/// range syntax uses, with each `"` and `\` in it doubled.
fn push_range_bound(text: &mut String, bound: &str) {
    let quoted = bound.is_empty()
        || bound.chars().any(|c| {
            matches!(c, '"' | '\\' | '(' | ')' | '[' | ']' | ',') || c.is_ascii_whitespace()
        });
    if quoted {
        text.push('"');
    }
    for c in bound.chars() {
        if matches!(c, '"' | '\\') {
            text.push(c);
        }
        text.push(c);
    }
    if quoted {
        text.push('"');
    }
}

pub(super) fn multirange_text(
    bytes: &[u8],
    bound: impl Fn(&[u8]) -> Option<String>,
) -> Option<String> {
    let mut rest = bytes;
    let count = usize::try_from(take_i32(&mut rest)?).ok()?;
    if count > rest.len() / 5 {
        return None;
    }
    let mut ranges = Vec::with_capacity(count);
    for _ in 0..count {
        ranges.push(range_text(take_value(&mut rest)??, &bound)?);
    }
    rest.is_empty().then(|| format!("{{{}}}", ranges.join(",")))
}
