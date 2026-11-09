//! Single values read from the database, owned or borrowed from a row.

use crate::database::Database;
use crate::decode::Decode;
use crate::error::{mismatched_types, Error};
use crate::type_info::TypeInfo;
use crate::types::Type;
use std::borrow::Cow;

/// A value read from the database that owns its data, detached from any row.
pub trait Value {
    type Database: Database<Value = Self>;

    /// Borrows the value, as a row would hand it out.
    fn as_ref(&self) -> <Self::Database as Database>::ValueRef<'_>;

    /// The value's type as the backend reports it.
    fn type_info(&self) -> Cow<'_, <Self::Database as Database>::TypeInfo>;

    /// Whether the value is SQL `NULL`.
    fn is_null(&self) -> bool;

    /// Decodes the value as `T`, checking first that `T` accepts the value's SQL type.
    ///
    /// # Panics
    ///
    /// When the value does not decode as `T`; [`try_decode`](Self::try_decode) returns
    /// the error instead.
    #[inline]
    fn decode<'r, T>(&'r self) -> T
    where
        T: Decode<'r, Self::Database> + Type<Self::Database>,
    {
        self.try_decode::<T>().unwrap()
    }

    /// Decodes the value as `T` without checking that `T` accepts its SQL type.
    ///
    /// # Panics
    ///
    /// When the value does not decode as `T`;
    /// [`try_decode_unchecked`](Self::try_decode_unchecked) returns the error instead.
    #[inline]
    fn decode_unchecked<'r, T>(&'r self) -> T
    where
        T: Decode<'r, Self::Database>,
    {
        self.try_decode_unchecked::<T>().unwrap()
    }

    /// Decodes the value as `T`, checking first that `T` accepts the value's SQL type.
    /// A `NULL`, or a value whose type the backend reports as null, skips the check.
    ///
    /// # Errors
    ///
    /// [`Error::Decode`] when the types do not match or decoding fails.
    #[inline]
    fn try_decode<'r, T>(&'r self) -> Result<T, Error>
    where
        T: Decode<'r, Self::Database> + Type<Self::Database>,
    {
        if !self.is_null() {
            let ty = self.type_info();

            if !ty.is_null() && !T::compatible(&ty) {
                return Err(Error::Decode(mismatched_types::<Self::Database, T>(&ty)));
            }
        }

        self.try_decode_unchecked()
    }

    /// Decodes the value as `T` without checking that `T` accepts its SQL type.
    ///
    /// # Errors
    ///
    /// [`Error::Decode`] when decoding fails.
    #[inline]
    fn try_decode_unchecked<'r, T>(&'r self) -> Result<T, Error>
    where
        T: Decode<'r, Self::Database>,
    {
        T::decode(self.as_ref()).map_err(Error::Decode)
    }
}

/// A value borrowed from a row; what [`Decode`] implementations read.
pub trait ValueRef<'r>: Sized {
    type Database: Database;

    /// Copies the value out of the row. PostgreSQL and MySQL share the row's buffer, so
    /// this costs a reference count there; SQLite copies the value.
    fn to_owned(&self) -> <Self::Database as Database>::Value;

    /// The value's type as the backend reports it.
    fn type_info(&self) -> Cow<'_, <Self::Database as Database>::TypeInfo>;

    /// Whether the value is SQL `NULL`.
    fn is_null(&self) -> bool;
}
