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

/// ## Postgres Geometric Path type
///
/// Description: Open path or Closed path (similar to polygon)
/// Representation: Open `[(x1,y1),...]`, Closed `((x1,y1),...)`
///
/// Paths are represented by lists of connected points. Paths can be open, where the first and last points in the list are considered not connected, or closed, where the first and last points are considered connected.
/// Values of type path are specified using any of the following syntaxes:
/// ```text
/// [ ( x1 , y1 ) , ... , ( xn , yn ) ]
/// ( ( x1 , y1 ) , ... , ( xn , yn ) )
///   ( x1 , y1 ) , ... , ( xn , yn )
///   ( x1 , y1   , ... ,   xn , yn )
///     x1 , y1   , ... ,   xn , yn
/// ```
/// where the points are the end points of the line segments comprising the path. Square brackets `([])` indicate an open path, while parentheses `(())` indicate a closed path.
/// When the outermost parentheses are omitted, as in the third through fifth syntaxes, a closed path is assumed.
///
/// See [Postgres Manual, Section 8.8.5, Geometric Types - Paths][PG.S.8.8.5] for details.
///
/// [PG.S.8.8.5]: https://www.postgresql.org/docs/current/datatype-geometric.html#DATATYPE-GEOMETRIC-PATHS
///
#[derive(Debug, Clone, PartialEq)]
pub struct PgPath {
    pub closed: bool,
    pub points: Vec<PgPoint>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Header {
    is_closed: bool,
    length: usize,
}

impl Type<Postgres> for PgPath {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::PATH
    }
}

impl PgHasArrayType for PgPath {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::PATH_ARRAY
    }
}

impl<'r> Decode<'r, Postgres> for PgPath {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        match value.format() {
            PgValueFormat::Text => Ok(PgPath::from_str(value.as_str()?)?),
            PgValueFormat::Binary => Ok(PgPath::from_bytes(value.as_bytes()?)?),
        }
    }
}

impl Encode<'_, Postgres> for PgPath {
    fn produces(&self) -> Option<PgTypeInfo> {
        Some(PgTypeInfo::PATH)
    }

    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        self.serialize(buf)?;
        Ok(IsNull::No)
    }
}

impl FromStr for PgPath {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let closed = !s.contains('[');
        let sanitised = s.replace(['(', ')', '[', ']', ' '], "");
        let parts = sanitised.split(',').collect::<Vec<_>>();

        let mut points = vec![];

        if parts.len() % 2 != 0 {
            return Err(Error::Decode(
                format!("Unmatched pair in PATH: {}", s).into(),
            ));
        }

        for [x_str, y_str] in parts.as_chunks::<2>().0 {
            let x = parse_float_from_str(x_str, "could not get x")?;
            let y = parse_float_from_str(y_str, "could not get y")?;

            points.push(PgPoint { x, y });
        }

        if !points.is_empty() {
            return Ok(PgPath { points, closed });
        }

        Err(Error::Decode(
            format!("could not get path from {}", s).into(),
        ))
    }
}

impl PgPath {
    fn header(&self) -> Header {
        Header {
            is_closed: self.closed,
            length: self.points.len(),
        }
    }

    fn from_bytes(mut bytes: &[u8]) -> Result<Self, BoxDynError> {
        let header = Header::try_read(&mut bytes)?;
        Ok(PgPath {
            closed: header.is_closed,
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
                "expected PATH data to contain at least {} bytes, got {}",
                Self::HEADER_WIDTH,
                buf.len()
            ));
        }

        let is_closed = buf.get_i8();
        let length = buf.get_i32();

        let length = usize::try_from(length).ok().ok_or_else(|| {
            format!(
                "received PATH data length: {length}. Expected length between 0 and {}",
                usize::MAX
            )
        })?;

        Ok(Self {
            is_closed: is_closed != 0,
            length,
        })
    }

    fn try_write(&self, buff: &mut PgArgumentBuffer) -> Result<(), String> {
        let is_closed = self.is_closed as i8;

        let length = i32::try_from(self.length).map_err(|_| {
            format!(
                "PATH length exceeds allowed maximum ({} > {})",
                self.length,
                i32::MAX
            )
        })?;

        buff.extend(is_closed.to_be_bytes());
        buff.extend(length.to_be_bytes());

        Ok(())
    }
}

fn parse_float_from_str(s: &str, error_msg: &str) -> Result<f64, Error> {
    s.parse().map_err(|_| Error::Decode(error_msg.into()))
}

#[cfg(test)]
#[path = "../../../tests/unit/types_geometry_path_tests.rs"]
mod path_tests;
