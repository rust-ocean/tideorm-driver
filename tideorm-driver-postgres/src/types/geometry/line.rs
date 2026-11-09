use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use driver_core::bytes::Buf;
use std::str::FromStr;

const ERROR: &str = "error decoding LINE";

/// ## Postgres Geometric Line type
///
/// Description: Infinite line
/// Representation: `{A, B, C}`
///
/// Lines are represented by the linear equation Ax + By + C = 0, where A and B are not both zero.
///
/// See [Postgres Manual, Section 8.8.2, Geometric Types - Lines][PG.S.8.8.2] for details.
///
/// [PG.S.8.8.2]: https://www.postgresql.org/docs/current/datatype-geometric.html#DATATYPE-LINE
///
#[derive(Debug, Clone, PartialEq)]
pub struct PgLine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}

impl Type<Postgres> for PgLine {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::LINE
    }
}

impl PgHasArrayType for PgLine {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::LINE_ARRAY
    }
}

impl<'r> Decode<'r, Postgres> for PgLine {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        match value.format() {
            PgValueFormat::Text => Ok(PgLine::from_str(value.as_str()?)?),
            PgValueFormat::Binary => Ok(PgLine::from_bytes(value.as_bytes()?)?),
        }
    }
}

impl Encode<'_, Postgres> for PgLine {
    fn produces(&self) -> Option<PgTypeInfo> {
        Some(PgTypeInfo::LINE)
    }

    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        self.serialize(buf);
        Ok(IsNull::No)
    }
}

impl FromStr for PgLine {
    type Err = BoxDynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s
            .trim_matches(|c| c == '{' || c == '}' || c == ' ')
            .split(',');

        let a = parts
            .next()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .ok_or_else(|| format!("{}: could not get a from {}", ERROR, s))?;

        let b = parts
            .next()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .ok_or_else(|| format!("{}: could not get b from {}", ERROR, s))?;

        let c = parts
            .next()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .ok_or_else(|| format!("{}: could not get c from {}", ERROR, s))?;

        if parts.next().is_some() {
            return Err(format!("{}: too many numbers inputted in {}", ERROR, s).into());
        }

        Ok(PgLine { a, b, c })
    }
}

impl PgLine {
    fn from_bytes(mut bytes: &[u8]) -> Result<PgLine, BoxDynError> {
        let a = bytes.get_f64();
        let b = bytes.get_f64();
        let c = bytes.get_f64();
        Ok(PgLine { a, b, c })
    }

    fn serialize(&self, buff: &mut PgArgumentBuffer) {
        buff.extend_from_slice(&self.a.to_be_bytes());
        buff.extend_from_slice(&self.b.to_be_bytes());
        buff.extend_from_slice(&self.c.to_be_bytes());
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/types_geometry_line_tests.rs"]
mod line_tests;
