//! Booleans, floats, text and blobs, and the `Json`/`Text` wrappers stored as text.
//!
//! SQLite stores a value as one of five classes (NULL, INTEGER, REAL, TEXT, BLOB); a
//! `bool` binds as an integer and a float as a double. The tables below list, per Rust
//! type, the type it reports and accepts, the argument it binds as, and how it reads a
//! value back. See <https://www.sqlite.org/datatype3.html>.

use std::borrow::Cow;
use std::fmt::Display;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::arguments::SqliteArgumentsBuffer;
use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::types::{Json, Text, Type};
use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

/// `Type`: the type a Rust type reports, then any other column types it decodes from.
macro_rules! impl_type {
    ($($ty:ty => $info:ident $(| $also:ident)*;)*) => {$(
        impl Type<Sqlite> for $ty {
            fn type_info() -> SqliteTypeInfo {
                SqliteTypeInfo(DataType::$info)
            }

            fn compatible(ty: &SqliteTypeInfo) -> bool {
                matches!(ty.0, DataType::$info $(| DataType::$also)*)
            }
        }
    )*};
}

impl_type! {
    bool => Bool | Int4 | Integer;
    f32 => Float;
    f64 => Float;
    str => Text;
    String => Text;
    [u8] => Blob | Text;
    Vec<u8> => Blob | Text;
}

/// `Encode`: the argument a value binds as, built from a reference `v`, and after `move`
/// the one built from the value itself, for types where owning it saves a copy.
macro_rules! impl_encode {
    ($(
        $ty:ty => |$v:ident| $variant:ident($by_ref:expr)
            $(, move $owned_variant:ident($owned:expr))?;
    )*) => {$(
        impl Encode<'_, Sqlite> for $ty {
            fn encode_by_ref(
                &self,
                args: &mut SqliteArgumentsBuffer,
            ) -> Result<IsNull, BoxDynError> {
                let $v = self;
                args.push(SqliteArgumentValue::$variant($by_ref));

                Ok(IsNull::No)
            }

            $(
                fn encode(self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
                    let $v = self;
                    args.push(SqliteArgumentValue::$owned_variant($owned));

                    Ok(IsNull::No)
                }
            )?
        }
    )*};
}

impl_encode! {
    bool => |v| Int((*v).into());
    f32 => |v| Double((*v).into());
    f64 => |v| Double(*v);

    &'_ str => |v| Text(Arc::new(v.to_string()));
    Box<str> => |v| Text(Arc::new(v.to_string())), move TextSlice(Arc::from(v));
    String => |v| Text(Arc::new(v.clone())), move Text(Arc::new(v));
    Cow<'_, str> => |v| Text(Arc::new(v.to_string())), move Text(Arc::new(v.into()));
    Arc<str> => |v| Text(Arc::new(v.to_string())), move TextSlice(v);
    Rc<str> => |v| Text(Arc::new(v.to_string()));

    &'_ [u8] => |v| Blob(Arc::new(v.to_vec()));
    Box<[u8]> => |v| Blob(Arc::new(v.to_vec())), move Blob(Arc::new(v.into_vec()));
    Vec<u8> => |v| Blob(Arc::new(v.clone())), move Blob(Arc::new(v));
    Cow<'_, [u8]> => |v| Blob(Arc::new(v.to_vec())), move Blob(Arc::new(v.into()));
    Arc<[u8]> => |v| Blob(Arc::new(v.to_vec()));
    Rc<[u8]> => |v| Blob(Arc::new(v.to_vec()));
}

/// `Decode`: what a type reads from the value `v`.
macro_rules! impl_decode {
    ($($(#[$attr:meta])* $ty:ty => |$v:ident| $read:expr;)*) => {$(
        $(#[$attr])*
        impl<'r> Decode<'r, Sqlite> for $ty {
            fn decode($v: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
                Ok($read)
            }
        }
    )*};
}

impl_decode! {
    bool => |v| v.int64()? != 0;
    // SQLite keeps only doubles; an `f32` takes the nearest value.
    #[allow(clippy::cast_possible_truncation)]
    f32 => |v| v.double()? as f32;
    f64 => |v| v.double()?;
    &'r str => |v| v.text_borrowed()?;
    String => |v| v.text_owned()?;
    &'r [u8] => |v| v.blob_borrowed();
    Vec<u8> => |v| v.blob_owned();
}

/// A JSON document stored as text.
impl<T> Type<Sqlite> for Json<T> {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Text)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <&str as Type<Sqlite>>::compatible(ty)
    }
}

impl<T> Encode<'_, Sqlite> for Json<T>
where
    T: Serialize,
{
    fn encode_by_ref(&self, buf: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        Encode::<Sqlite>::encode(self.encode_to_string()?, buf)
    }
}

impl<'r, T> Decode<'r, Sqlite> for Json<T>
where
    T: 'r + Deserialize<'r>,
{
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        // Hand serde_json the bytes: it checks the UTF-8 as it parses, in the same pass.
        Self::decode_from_bytes(Decode::<Sqlite>::decode(value)?)
    }
}

/// A value stored as its `Display` text and read back with `FromStr`.
impl<T> Type<Sqlite> for Text<T> {
    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }
}

impl<T> Encode<'_, Sqlite> for Text<T>
where
    T: Display,
{
    fn encode_by_ref(&self, buf: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        Encode::<Sqlite>::encode(self.0.to_string(), buf)
    }
}

impl<'r, T> Decode<'r, Sqlite> for Text<T>
where
    T: FromStr,
    BoxDynError: From<<T as FromStr>::Err>,
{
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(Self(value.with_temp_text(|text| text.parse::<T>())??))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/scalar_tests.rs"]
mod tests;
