use crate::types::numeric::{
    PgNumeric, RawNumeric, SIGN_NAN, SIGN_NEG, SIGN_NINF, SIGN_PINF, SIGN_POS,
};
use std::fmt::Write;

/// The decimal text of a PostgreSQL `NUMERIC` in its binary wire form.
pub(super) fn numeric_text(bytes: &[u8]) -> Option<String> {
    let RawNumeric {
        weight,
        sign,
        scale,
        digits,
    } = RawNumeric::read(bytes)?;
    match sign {
        SIGN_NAN => return Some("NaN".to_string()),
        SIGN_PINF => return Some("Infinity".to_string()),
        SIGN_NINF => return Some("-Infinity".to_string()),
        SIGN_POS | SIGN_NEG => {}
        _ => return None,
    }
    let weight = i32::from(weight);
    let scale = usize::try_from(scale).ok()?;
    if !digits.iter().copied().all(PgNumeric::is_valid_digit) {
        return None;
    }
    // The digit of base-10000 place `weight - group`; zero past the stored ones.
    let digit = |group: i32| {
        usize::try_from(group)
            .ok()
            .and_then(|group| digits.get(group).copied())
            .unwrap_or(0)
    };

    let mut text = String::new();
    if sign == SIGN_NEG {
        text.push('-');
    }
    if weight < 0 {
        text.push('0');
    } else {
        let _ = write!(text, "{}", digit(0));
        for group in 1..=weight {
            let _ = write!(text, "{:04}", digit(group));
        }
    }
    if scale > 0 {
        text.push('.');
        let point = text.len();
        let mut group = weight + 1;
        while text.len() - point < scale {
            let _ = write!(text, "{:04}", digit(group));
            group += 1;
        }
        text.truncate(point + scale);
    }
    Some(text)
}
