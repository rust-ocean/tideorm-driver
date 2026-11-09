use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use driver_core::bytes::Buf;
use std::str::FromStr;

use super::parse_four_numbers;

const ERROR: &str = "error decoding BOX";

/// ## Postgres Geometric Box type
///
/// Description: Rectangular box
/// Representation: `((upper_right_x,upper_right_y),(lower_left_x,lower_left_y))`
///
/// Boxes are represented by pairs of points that are opposite corners of the box. Values of type box are specified using any of the following syntaxes:
///
/// ```text
/// ( ( upper_right_x , upper_right_y ) , ( lower_left_x , lower_left_y ) )
/// ( upper_right_x , upper_right_y ) , ( lower_left_x , lower_left_y )
///   upper_right_x , upper_right_y   ,   lower_left_x , lower_left_y
/// ```
/// where `(upper_right_x,upper_right_y) and (lower_left_x,lower_left_y)` are any two opposite corners of the box.
/// Any two opposite corners can be supplied on input, but the values will be reordered as needed to store the upper right and lower left corners, in that order.
///
/// See [Postgres Manual, Section 8.8.4: Geometric Types - Boxes][PG.S.8.8.4] for details.
///
/// [PG.S.8.8.4]: https://www.postgresql.org/docs/current/datatype-geometric.html#DATATYPE-GEOMETRIC-BOXES
///
#[derive(Debug, Clone, PartialEq)]
pub struct PgBox {
    pub upper_right_x: f64,
    pub upper_right_y: f64,
    pub lower_left_x: f64,
    pub lower_left_y: f64,
}

impl Type<Postgres> for PgBox {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::BOX
    }
}

impl PgHasArrayType for PgBox {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::BOX_ARRAY
    }
}

impl<'r> Decode<'r, Postgres> for PgBox {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        match value.format() {
            PgValueFormat::Text => Ok(PgBox::from_str(value.as_str()?)?),
            PgValueFormat::Binary => Ok(PgBox::from_bytes(value.as_bytes()?)?),
        }
    }
}

impl Encode<'_, Postgres> for PgBox {
    fn produces(&self) -> Option<PgTypeInfo> {
        Some(PgTypeInfo::BOX)
    }

    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        self.serialize(buf);
        Ok(IsNull::No)
    }
}

impl FromStr for PgBox {
    type Err = BoxDynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let [upper_right_x, upper_right_y, lower_left_x, lower_left_y] = parse_four_numbers(
            s,
            ERROR,
            [
                "upper_right_x",
                "upper_right_y",
                "lower_left_x",
                "lower_left_y",
            ],
        )?;

        Ok(PgBox {
            upper_right_x,
            upper_right_y,
            lower_left_x,
            lower_left_y,
        })
    }
}

impl PgBox {
    fn from_bytes(mut bytes: &[u8]) -> Result<PgBox, BoxDynError> {
        let upper_right_x = bytes.get_f64();
        let upper_right_y = bytes.get_f64();
        let lower_left_x = bytes.get_f64();
        let lower_left_y = bytes.get_f64();

        Ok(PgBox {
            upper_right_x,
            upper_right_y,
            lower_left_x,
            lower_left_y,
        })
    }

    fn serialize(&self, buff: &mut PgArgumentBuffer) {
        let min_x = &self.upper_right_x.min(self.lower_left_x);
        let min_y = &self.upper_right_y.min(self.lower_left_y);
        let max_x = &self.upper_right_x.max(self.lower_left_x);
        let max_y = &self.upper_right_y.max(self.lower_left_y);

        buff.extend_from_slice(&max_x.to_be_bytes());
        buff.extend_from_slice(&max_y.to_be_bytes());
        buff.extend_from_slice(&min_x.to_be_bytes());
        buff.extend_from_slice(&min_y.to_be_bytes());
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/types_geometry_box_tests.rs"]
mod box_tests;
