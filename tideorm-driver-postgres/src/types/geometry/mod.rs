use std::mem;

use driver_core::bytes::Buf;

use crate::error::BoxDynError;
use crate::types::PgPoint;

pub mod r#box;
pub mod circle;
pub mod line;
pub mod line_segment;
pub mod path;
pub mod point;
pub mod polygon;

const BYTE_WIDTH: usize = mem::size_of::<f64>();

/// Reads the four numbers of a BOX or LSEG in text form; `error` and the
/// field `names` make up its messages.
fn parse_four_numbers(s: &str, error: &str, names: [&str; 4]) -> Result<[f64; 4], BoxDynError> {
    let sanitised = s.replace(['(', ')', '[', ']', ' '], "");
    let mut parts = sanitised.split(',');
    let mut numbers = [0.0; 4];

    for (number, name) in numbers.iter_mut().zip(names) {
        *number = parts
            .next()
            .and_then(|part| part.parse::<f64>().ok())
            .ok_or_else(|| format!("{error}: could not get {name} from {s}"))?;
    }

    if parts.next().is_some() {
        return Err(format!("{error}: too many numbers inputted in {s}").into());
    }

    Ok(numbers)
}

/// Reads the `(x, y)` pairs after a PATH or POLYGON header that announced `data_size` bytes.
fn read_points(mut bytes: &[u8], data_size: usize) -> Result<Vec<PgPoint>, BoxDynError> {
    if bytes.len() != data_size {
        return Err(format!(
            "expected {} bytes after header, got {}",
            data_size,
            bytes.len()
        )
        .into());
    }

    if !bytes.len().is_multiple_of(BYTE_WIDTH * 2) {
        return Err(format!(
            "data length not divisible by pairs of {BYTE_WIDTH}: {}",
            bytes.len()
        )
        .into());
    }

    let mut out_points = Vec::with_capacity(bytes.len() / (BYTE_WIDTH * 2));
    while bytes.has_remaining() {
        let point = PgPoint {
            x: bytes.get_f64(),
            y: bytes.get_f64(),
        };
        out_points.push(point)
    }
    Ok(out_points)
}
