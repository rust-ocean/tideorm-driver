//! Text: Rust strings for PostgreSQL's string types, and [`Text<T>`] for any value kept as the
//! string it displays as. Both formats carry a string as its UTF-8 bytes.

use std::borrow::Cow;
use std::fmt::Display;
use std::io::Write;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use driver_core::types::Text;

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgTypeInfo, PgValueRef, Postgres};

impl Type<Postgres> for str {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::TEXT
    }

    // `UNKNOWN` is the type of an untyped literal (`SELECT 'a'`); `CITEXT` is an extension's
    // type, known only by name.
    fn compatible(ty: &PgTypeInfo) -> bool {
        [
            PgTypeInfo::TEXT,
            PgTypeInfo::NAME,
            PgTypeInfo::BPCHAR,
            PgTypeInfo::VARCHAR,
            PgTypeInfo::UNKNOWN,
            PgTypeInfo::with_name("citext"),
        ]
        .contains(ty)
    }
}

impl Type<Postgres> for String {
    fn type_info() -> PgTypeInfo {
        <str as Type<Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <str as Type<Postgres>>::compatible(ty)
    }
}

array_type!(TEXT_ARRAY of str: [] &'_ str, [] Cow<'_, str>, [] Box<str>, [] String);

impl Encode<'_, Postgres> for &'_ str {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend_from_slice(self.as_bytes());
        Ok(IsNull::No)
    }
}

forward_encode_impl!(Arc<str>, &str, Postgres);
forward_encode_impl!(Rc<str>, &str, Postgres);
forward_encode_impl!(Cow<'_, str>, &str, Postgres);
forward_encode_impl!(Box<str>, &str, Postgres);
forward_encode_impl!(String, &str, Postgres);

impl<'r> Decode<'r, Postgres> for &'r str {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        value.as_str()
    }
}

impl Decode<'_, Postgres> for String {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(value.as_str()?.to_owned())
    }
}

impl<T> Type<Postgres> for Text<T> {
    fn type_info() -> PgTypeInfo {
        <str as Type<Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <str as Type<Postgres>>::compatible(ty)
    }
}

impl<T: Display> Encode<'_, Postgres> for Text<T> {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        write!(**buf, "{}", self.0)?;
        Ok(IsNull::No)
    }
}

impl<T> Decode<'_, Postgres> for Text<T>
where
    T: FromStr,
    BoxDynError: From<T::Err>,
{
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(Self(value.as_str()?.parse()?))
    }
}
