use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::{PgPoint, Type};
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use driver_core::bytes::Buf;
use driver_core::Error;
use std::mem;
use std::str::FromStr;

use super::{read_points, BYTE_WIDTH};

/// ## Postgres Geometric Polygon type
///
/// Description: Polygon (similar to closed polygon)
/// Representation: `((x1,y1),...)`
///
/// Polygons are represented by lists of points (the vertices of the polygon). Polygons are very similar to closed paths; the essential semantic difference is that a polygon is considered to include the area within it, while a path is not.
/// An important implementation difference between polygons and paths is that the stored representation of a polygon includes its smallest bounding box. This speeds up certain search operations, although computing the bounding box adds overhead while constructing new polygons.
/// Values of type polygon are specified using any of the following syntaxes:
///
/// ```text
/// ( ( x1 , y1 ) , ... , ( xn , yn ) )
///   ( x1 , y1 ) , ... , ( xn , yn )
///   ( x1 , y1   , ... ,   xn , yn )
///     x1 , y1   , ... ,   xn , yn
/// ```
///
/// where the points are the end points of the line segments comprising the boundary of the polygon.
///
/// See [Postgres Manual, Section 8.8.6, Geometric Types - Polygons][PG.S.8.8.6] for details.
///
/// [PG.S.8.8.6]: https://www.postgresql.org/docs/current/datatype-geometric.html#DATATYPE-POLYGON
///
#[derive(Debug, Clone, PartialEq)]
pub struct PgPolygon {
    pub points: Vec<PgPoint>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Header {
    length: usize,
}

impl Type<Postgres> for PgPolygon {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::POLYGON
    }
}

impl PgHasArrayType for PgPolygon {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::POLYGON_ARRAY
    }
}

impl<'r> Decode<'r, Postgres> for PgPolygon {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        match value.format() {
            PgValueFormat::Text => Ok(PgPolygon::from_str(value.as_str()?)?),
            PgValueFormat::Binary => Ok(PgPolygon::from_bytes(value.as_bytes()?)?),
        }
    }
}

impl Encode<'_, Postgres> for PgPolygon {
    fn produces(&self) -> Option<PgTypeInfo> {
        Some(PgTypeInfo::POLYGON)
    }

    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        self.serialize(buf)?;
        Ok(IsNull::No)
    }
}

impl FromStr for PgPolygon {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let sanitised = s.replace(['(', ')', '[', ']', ' '], "");
        let parts = sanitised.split(',').collect::<Vec<_>>();

        let mut points = vec![];

        if parts.len() % 2 != 0 {
            return Err(Error::Decode(
                format!("Unmatched pair in POLYGON: {}", s).into(),
            ));
        }

        for [x_str, y_str] in parts.as_chunks::<2>().0 {
            let x = parse_float_from_str(x_str, "could not get x")?;
            let y = parse_float_from_str(y_str, "could not get y")?;

            points.push(PgPoint { x, y });
        }

        if !points.is_empty() {
            return Ok(PgPolygon { points });
        }

        Err(Error::Decode(
            format!("could not get polygon from {}", s).into(),
        ))
    }
}

impl PgPolygon {
    fn header(&self) -> Header {
        Header {
            length: self.points.len(),
        }
    }

    fn from_bytes(mut bytes: &[u8]) -> Result<Self, BoxDynError> {
        let header = Header::try_read(&mut bytes)?;
        Ok(PgPolygon {
            points: read_points(bytes, header.data_size())?,
        })
    }

    fn serialize(&self, buff: &mut PgArgumentBuffer) -> Result<(), BoxDynError> {
        let header = self.header();
        buff.reserve(header.data_size());
        header.try_write(buff)?;

        for point in &self.points {
            buff.extend_from_slice(&point.x.to_be_bytes());
            buff.extend_from_slice(&point.y.to_be_bytes());
        }
        Ok(())
    }
}

impl Header {
    const HEADER_WIDTH: usize = mem::size_of::<i8>() + mem::size_of::<i32>();

    fn data_size(&self) -> usize {
        self.length * BYTE_WIDTH * 2
    }

    fn try_read(buf: &mut &[u8]) -> Result<Self, String> {
        if buf.len() < Self::HEADER_WIDTH {
            return Err(format!(
                "expected polygon data to contain at least {} bytes, got {}",
                Self::HEADER_WIDTH,
                buf.len()
            ));
        }

        let length = buf.get_i32();

        let length = usize::try_from(length).ok().ok_or_else(|| {
            format!(
                "received polygon with length: {length}. Expected length between 0 and  {}",
                usize::MAX
            )
        })?;

        Ok(Self { length })
    }

    fn try_write(&self, buff: &mut PgArgumentBuffer) -> Result<(), String> {
        let length = i32::try_from(self.length).map_err(|_| {
            format!(
                "polygon length exceeds allowed maximum ({} > {})",
                self.length,
                i32::MAX
            )
        })?;

        buff.extend(length.to_be_bytes());

        Ok(())
    }
}

fn parse_float_from_str(s: &str, error_msg: &str) -> Result<f64, Error> {
    s.parse().map_err(|_| Error::Decode(error_msg.into()))
}

#[cfg(test)]
#[path = "../../../tests/unit/types_geometry_polygon_tests.rs"]
mod polygon_tests;
