use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use driver_core::bytes::Buf;
use std::str::FromStr;

use super::parse_four_numbers;

const ERROR: &str = "error decoding LSEG";

/// ## Postgres Geometric Line Segment type
///
/// Description: Finite line segment
/// Representation: `((start_x,start_y),(end_x,end_y))`
///
///
/// Line segments are represented by pairs of points that are the endpoints of the segment. Values of type lseg are specified using any of the following syntaxes:
/// ```text
/// [ ( start_x , start_y ) , ( end_x , end_y ) ]
/// ( ( start_x , start_y ) , ( end_x , end_y ) )
///   ( start_x , start_y ) , ( end_x , end_y )
///     start_x , start_y   ,   end_x , end_y
/// ```
/// where `(start_x,start_y) and (end_x,end_y)` are the end points of the line segment.
///
/// See [Postgres Manual, Section 8.8.3, Geometric Types - Line Segments][PG.S.8.8.3] for details.
///
/// [PG.S.8.8.3]: https://www.postgresql.org/docs/current/datatype-geometric.html#DATATYPE-LSEG
///
#[doc(alias = "line segment")]
#[derive(Debug, Clone, PartialEq)]
pub struct PgLSeg {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
}

impl Type<Postgres> for PgLSeg {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::LSEG
    }
}

impl PgHasArrayType for PgLSeg {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::LSEG_ARRAY
    }
}

impl<'r> Decode<'r, Postgres> for PgLSeg {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        match value.format() {
            PgValueFormat::Text => Ok(PgLSeg::from_str(value.as_str()?)?),
            PgValueFormat::Binary => Ok(PgLSeg::from_bytes(value.as_bytes()?)?),
        }
    }
}

impl Encode<'_, Postgres> for PgLSeg {
    fn produces(&self) -> Option<PgTypeInfo> {
        Some(PgTypeInfo::LSEG)
    }

    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        self.serialize(buf);
        Ok(IsNull::No)
    }
}

impl FromStr for PgLSeg {
    type Err = BoxDynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let [start_x, start_y, end_x, end_y] =
            parse_four_numbers(s, ERROR, ["start_x", "start_y", "end_x", "end_y"])?;

        Ok(PgLSeg {
            start_x,
            start_y,
            end_x,
            end_y,
        })
    }
}

impl PgLSeg {
    fn from_bytes(mut bytes: &[u8]) -> Result<PgLSeg, BoxDynError> {
        let start_x = bytes.get_f64();
        let start_y = bytes.get_f64();
        let end_x = bytes.get_f64();
        let end_y = bytes.get_f64();

        Ok(PgLSeg {
            start_x,
            start_y,
            end_x,
            end_y,
        })
    }

    fn serialize(&self, buff: &mut PgArgumentBuffer) {
        buff.extend_from_slice(&self.start_x.to_be_bytes());
        buff.extend_from_slice(&self.start_y.to_be_bytes());
        buff.extend_from_slice(&self.end_x.to_be_bytes());
        buff.extend_from_slice(&self.end_y.to_be_bytes());
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/types_geometry_line_segment_tests.rs"]
mod lseg_tests;
