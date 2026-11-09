use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
pub use serde_json::value::RawValue as JsonRawValue;
pub use serde_json::Value as JsonValue;

use crate::database::Database;
use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;

/// Json for json and jsonb fields.
///
/// Attempts to deserialize the JSON value into the generic type.
///
/// # Example
///
/// ```rust
/// use driver::{Row, postgres::PgRow, types::Json};
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Book {
///     name: String,
/// }
///
/// struct Author {
///     name: String,
///     books: Json<Book>,
/// }
///
/// fn author(row: &PgRow) -> driver::Result<Author> {
///     Ok(Author {
///         name: row.try_get("name")?,
///         books: row.try_get("books")?,
///     })
/// }
/// ```
///
/// It can also deserialize JSON into a hash map:
/// ```rust
/// use std::collections::HashMap;
///
/// use driver::{Row, postgres::PgRow, types::Json};
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Book {
///     name: String,
/// }
///
/// struct Library {
///     id: String,
///     dewey_decimal: Json<HashMap<String, Book>>,
/// }
///
/// fn library(row: &PgRow) -> driver::Result<Library> {
///     Ok(Library {
///         id: row.try_get("id")?,
///         dewey_decimal: row.try_get("dewey_decimal")?,
///     })
/// }
/// ```
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Json<T: ?Sized>(pub T);

impl<T> Json<T> {
    /// Extract the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for Json<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> AsRef<T> for Json<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for Json<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

// UNSTABLE: for driver use only!
#[doc(hidden)]
impl<T: Serialize> Json<T> {
    pub fn encode_to_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn encode_to(&self, buf: &mut Vec<u8>) -> Result<(), serde_json::Error> {
        serde_json::to_writer(buf, self)
    }
}

// UNSTABLE: for driver use only!
#[doc(hidden)]
impl<'a, T: 'a> Json<T>
where
    T: Deserialize<'a>,
{
    pub fn decode_from_string(s: &'a str) -> Result<Self, BoxDynError> {
        serde_json::from_str(s).map_err(Into::into)
    }

    pub fn decode_from_bytes(bytes: &'a [u8]) -> Result<Self, BoxDynError> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }
}

impl<DB> Type<DB> for JsonValue
where
    Json<Self>: Type<DB>,
    DB: Database,
{
    fn type_info() -> DB::TypeInfo {
        <Json<Self> as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <Json<Self> as Type<DB>>::compatible(ty)
    }
}

impl<'q, DB> Encode<'q, DB> for JsonValue
where
    for<'a> Json<&'a Self>: Encode<'q, DB>,
    DB: Database,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <Json<&Self> as Encode<'q, DB>>::encode(Json(self), buf)
    }
}

impl<'r, DB> Decode<'r, DB> for JsonValue
where
    Json<Self>: Decode<'r, DB>,
    DB: Database,
{
    fn decode(value: <DB as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        <Json<Self> as Decode<DB>>::decode(value).map(|item| item.0)
    }
}

impl<DB> Type<DB> for JsonRawValue
where
    for<'a> Json<&'a Self>: Type<DB>,
    DB: Database,
{
    fn type_info() -> DB::TypeInfo {
        <Json<&Self> as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <Json<&Self> as Type<DB>>::compatible(ty)
    }
}

impl<'q, DB> Encode<'q, DB> for JsonRawValue
where
    for<'a> Json<&'a Self>: Encode<'q, DB>,
    DB: Database,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <Json<&Self> as Encode<'q, DB>>::encode(Json(self), buf)
    }
}

impl<'q, DB> Encode<'q, DB> for &'q JsonRawValue
where
    for<'a> Json<&'a Self>: Encode<'q, DB>,
    DB: Database,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <Json<&Self> as Encode<'q, DB>>::encode(Json(self), buf)
    }
}

impl<'q, DB> Encode<'q, DB> for Box<JsonRawValue>
where
    for<'a> Json<&'a Self>: Encode<'q, DB>,
    DB: Database,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <Json<&Self> as Encode<'q, DB>>::encode(Json(self), buf)
    }
}

impl<'r, DB> Decode<'r, DB> for &'r JsonRawValue
where
    Json<Self>: Decode<'r, DB>,
    DB: Database,
{
    fn decode(value: <DB as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        <Json<Self> as Decode<DB>>::decode(value).map(|item| item.0)
    }
}

impl<'r, DB> Decode<'r, DB> for Box<JsonRawValue>
where
    Json<Self>: Decode<'r, DB>,
    DB: Database,
{
    fn decode(value: <DB as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        <Json<Self> as Decode<DB>>::decode(value).map(|item| item.0)
    }
}
