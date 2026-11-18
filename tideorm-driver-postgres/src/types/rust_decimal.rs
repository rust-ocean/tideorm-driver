use driver_core::types::decimal_from_text;
use rust_decimal::Decimal;

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::numeric::{PgNumeric, PgNumericSign};
use crate::types::Type;
use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

impl Type<Postgres> for Decimal {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::NUMERIC
    }
}

impl PgHasArrayType for Decimal {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::NUMERIC_ARRAY
    }
}

impl TryFrom<PgNumeric> for Decimal {
    type Error = BoxDynError;

    fn try_from(numeric: PgNumeric) -> Result<Self, BoxDynError> {
        Decimal::try_from(&numeric)
    }
}

impl TryFrom<&'_ PgNumeric> for Decimal {
    type Error = BoxDynError;

    fn try_from(numeric: &'_ PgNumeric) -> Result<Self, BoxDynError> {
        let (digits, sign, weight, scale) = match *numeric {
            PgNumeric::Number {
                ref digits,
                sign,
                weight,
                scale,
            } => (digits, sign, weight, scale),

            PgNumeric::NotANumber => {
                return Err("Decimal does not support NaN values".into());
            }
        };

        // zero groups ending the digits add nothing
        let digits = &digits[..digits
            .iter()
            .rposition(|&d| d != 0)
            .map_or(0, |last| last + 1)];
        if digits.is_empty() {
            // Postgres returns an empty digit array for 0
            return Ok(Decimal::ZERO);
        }

        let scale = u32::try_from(scale)
            .map_err(|_| format!("invalid scale value for Pg NUMERIC: {scale}"))?;

        // The digits as one exact integer: summing `Decimal`s would round once the value
        // needs more than the 28 digits one holds.
        const UNREPRESENTABLE: &str = "value not representable as rust_decimal::Decimal";
        let mut mantissa = 0_i128;
        for &digit in digits {
            mantissa = mantissa
                .checked_mul(10_000)
                .and_then(|m| m.checked_add(digit.into()))
                .ok_or(UNREPRESENTABLE)?;
        }
        // the power of ten of the last digit
        let digit_count = i64::try_from(digits.len()).map_err(|_| UNREPRESENTABLE)?;
        let mut exponent = 4 * (i64::from(weight) + 1 - digit_count);
        while exponent < 0 && mantissa % 10 == 0 {
            mantissa /= 10;
            exponent += 1;
        }
        if exponent > 0 {
            mantissa = u32::try_from(exponent)
                .ok()
                .and_then(|e| 10_i128.checked_pow(e))
                .and_then(|power| mantissa.checked_mul(power))
                .ok_or(UNREPRESENTABLE)?;
        }
        let fraction = u32::try_from(-exponent.min(0)).map_err(|_| UNREPRESENTABLE)?;
        let mut value =
            Decimal::try_from_i128_with_scale(mantissa, fraction).map_err(|_| UNREPRESENTABLE)?;

        match sign {
            PgNumericSign::Positive => value.set_sign_positive(true),
            PgNumericSign::Negative => value.set_sign_negative(true),
        }

        // the zeros of the declared scale; a smaller one would round
        if scale > value.scale() {
            value.rescale(scale);
        }

        Ok(value)
    }
}

impl From<Decimal> for PgNumeric {
    fn from(value: Decimal) -> Self {
        PgNumeric::from(&value)
    }
}

// This impl is effectively infallible because `NUMERIC` has a greater range than `Decimal`.
impl From<&'_ Decimal> for PgNumeric {
    // Impl has been manually validated.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn from(decimal: &Decimal) -> Self {
        if Decimal::is_zero(decimal) {
            return PgNumeric::ZERO;
        }

        assert!(
            (0u32..=28).contains(&decimal.scale()),
            "decimal scale out of range {:?}",
            decimal.unpack(),
        );

        // Cannot overflow: always in the range [0, 28]
        let scale = decimal.scale() as u16;

        let mut mantissa = decimal.mantissa().unsigned_abs();

        // If our scale is not a multiple of 4, we need to go to the next multiple.
        let groups_diff = scale % 4;
        if groups_diff > 0 {
            let remainder = 4 - groups_diff as u32;
            let power = 10u32.pow(remainder) as u128;

            // Impossible to overflow; 0 <= mantissa <= 2^96,
            // and we're multiplying by at most 1,000 (giving us a result < 2^106)
            mantissa *= power;
        }

        // Array to store max mantissa of Decimal in Postgres decimal format.
        let mut digits = Vec::with_capacity(8);

        // Convert to base-10000.
        while mantissa != 0 {
            // Cannot overflow or wrap because of the modulus
            digits.push((mantissa % 10_000) as i16);
            mantissa /= 10_000;
        }

        // We started with the low digits first, but they should actually be at the end.
        digits.reverse();

        // Cannot overflow: strictly smaller than `scale`.
        let digits_after_decimal = scale.div_ceil(4) as i16;

        // `mantissa` contains at most 29 decimal digits (log10(2^96)),
        // split into at most 8 4-digit segments.
        assert!(
            digits.len() <= 8,
            "digits.len() out of range: {}; unpacked: {:?}",
            digits.len(),
            decimal.unpack()
        );

        // Cannot overflow; at most 8
        let num_digits = digits.len() as i16;

        // Find how many 4-digit segments should go before the decimal point.
        // `weight = 0` puts just `digit[0]` before the decimal point, and the rest after.
        let weight = num_digits - digits_after_decimal - 1;

        // Remove non-significant zeroes.
        while let Some(&0) = digits.last() {
            digits.pop();
        }

        PgNumeric::Number {
            sign: match decimal.is_sign_negative() {
                false => PgNumericSign::Positive,
                true => PgNumericSign::Negative,
            },
            // Cannot overflow; between 0 and 28
            scale: scale as i16,
            weight,
            digits,
        }
    }
}

impl Encode<'_, Postgres> for Decimal {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        PgNumeric::from(self).encode(buf)?;

        Ok(IsNull::No)
    }
}

#[doc=include_str!("rust_decimal-range.md")]
impl Decode<'_, Postgres> for Decimal {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.format() {
            PgValueFormat::Binary => PgNumeric::decode(value.as_bytes()?)?.try_into(),
            PgValueFormat::Text => decimal_from_text(value.as_str()?),
        }
    }
}

#[cfg(test)]
#[allow(clippy::zero_prefixed_literal)] // Used for clarity
#[path = "../../tests/unit/types_rust_decimal_tests.rs"]
mod tests;
