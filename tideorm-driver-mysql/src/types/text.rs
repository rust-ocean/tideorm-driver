//! Values sent and read as text: [`Text<T>`], IP addresses and `DECIMAL`.
//!
//! Each is written with `Display` as a length-encoded string and read back
//! with `FromStr`, in either result format: the binary format sends
//! `DECIMAL` as its digits too.

use std::fmt::Display;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use rust_decimal::Decimal;

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::io::MySqlBufMutExt;
use crate::protocol::text::ColumnType;
use crate::types::{Text, Type};
use crate::{MySql, MySqlTypeInfo, MySqlValueRef};

fn encode_text(value: &impl Display, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
    buf.put_str_lenenc(&value.to_string());

    Ok(IsNull::No)
}

fn decode_text<T>(value: MySqlValueRef<'_>) -> Result<T, BoxDynError>
where
    T: FromStr,
    BoxDynError: From<T::Err>,
{
    Ok(value.as_str()?.parse()?)
}

impl<T> Type<MySql> for Text<T> {
    fn type_info() -> MySqlTypeInfo {
        <String as Type<MySql>>::type_info()
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        <String as Type<MySql>>::compatible(ty)
    }
}

impl<T> Encode<'_, MySql> for Text<T>
where
    T: Display,
{
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        encode_text(&self.0, buf)
    }
}

impl<'r, T> Decode<'r, MySql> for Text<T>
where
    T: FromStr,
    BoxDynError: From<<T as FromStr>::Err>,
{
    fn decode(value: MySqlValueRef<'r>) -> Result<Self, BoxDynError> {
        decode_text(value).map(Self)
    }
}

// MySQL has no address type, and MariaDB's `INET4`/`INET6` take and return
// text, so an address is a string column's value.
macro_rules! impl_ip_addr {
    ($($ty:ty),*) => {$(
        impl Type<MySql> for $ty {
            fn type_info() -> MySqlTypeInfo {
                <&str as Type<MySql>>::type_info()
            }

            fn compatible(ty: &MySqlTypeInfo) -> bool {
                <&str as Type<MySql>>::compatible(ty)
            }
        }

        impl Encode<'_, MySql> for $ty {
            fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
                encode_text(self, buf)
            }
        }

        impl Decode<'_, MySql> for $ty {
            fn decode(value: MySqlValueRef<'_>) -> Result<Self, BoxDynError> {
                decode_text(value)
            }
        }
    )*};
}

impl_ip_addr!(IpAddr, Ipv4Addr, Ipv6Addr);

impl Type<MySql> for Decimal {
    fn type_info() -> MySqlTypeInfo {
        MySqlTypeInfo::binary(ColumnType::NewDecimal)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        matches!(ty.r#type, ColumnType::Decimal | ColumnType::NewDecimal)
    }
}

impl Encode<'_, MySql> for Decimal {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        encode_text(self, buf)
    }
}

impl Decode<'_, MySql> for Decimal {
    fn decode(value: MySqlValueRef<'_>) -> Result<Self, BoxDynError> {
        decode_text(value)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/types_text_tests.rs"]
mod tests;
