use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use driver_core::bytes::Buf;
use driver_core::Error;
use std::str::FromStr;

/// ## Postgres Geometric Point type
///
/// Description: Point on a plane
/// Representation: `(x, y)`
///
/// Points are the fundamental two-dimensional building block for geometric types. Values of type point are specified using either of the following syntaxes:
/// ```text
/// ( x , y )
///  x , y
/// ````
/// where x and y are the respective coordinates, as floating-point numbers.
///
/// See [Postgres Manual, Section 8.8.1, Geometric Types - Points][PG.S.8.8.1] for details.
///
/// [PG.S.8.8.1]: https://www.postgresql.org/docs/current/datatype-geometric.html#DATATYPE-GEOMETRIC-POINTS
///
#[derive(Debug, Clone, PartialEq)]
pub struct PgPoint {
    pub x: f64,
    pub y: f64,
}

impl Type<Postgres> for PgPoint {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::POINT
    }
}

impl PgHasArrayType for PgPoint {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::POINT_ARRAY
    }
}

impl<'r> Decode<'r, Postgres> for PgPoint {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        match value.format() {
            PgValueFormat::Text => Ok(PgPoint::from_str(value.as_str()?)?),
            PgValueFormat::Binary => Ok(PgPoint::from_bytes(value.as_bytes()?)?),
        }
    }
}

impl Encode<'_, Postgres> for PgPoint {
    fn produces(&self) -> Option<PgTypeInfo> {
        Some(PgTypeInfo::POINT)
    }

    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        self.serialize(buf);
        Ok(IsNull::No)
    }
}

fn parse_float_from_str(s: &str, error_msg: &str) -> Result<f64, Error> {
    s.trim()
        .parse()
        .map_err(|_| Error::Decode(error_msg.into()))
}

impl FromStr for PgPoint {
    type Err = BoxDynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (x_str, y_str) = s
            .trim_matches(|c| c == '(' || c == ')' || c == ' ')
            .split_once(',')
            .ok_or_else(|| format!("error decoding POINT: could not get x and y from {}", s))?;

        let x = parse_float_from_str(x_str, "error decoding POINT: could not get x")?;
        let y = parse_float_from_str(y_str, "error decoding POINT: could not get y")?;

        Ok(PgPoint { x, y })
    }
}

impl PgPoint {
    fn from_bytes(mut bytes: &[u8]) -> Result<PgPoint, BoxDynError> {
        let x = bytes.get_f64();
        let y = bytes.get_f64();
        Ok(PgPoint { x, y })
    }

    fn serialize(&self, buff: &mut PgArgumentBuffer) {
        buff.extend_from_slice(&self.x.to_be_bytes());
        buff.extend_from_slice(&self.y.to_be_bytes());
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/types_geometry_point_tests.rs"]
mod point_tests;
