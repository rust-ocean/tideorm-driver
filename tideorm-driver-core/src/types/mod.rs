//! Conversions between Rust and SQL types.
//!
//! To see how each SQL type maps to a Rust type, see the corresponding `types` module for each
//! database:
//!
//!  * PostgreSQL: `driver::postgres::types`
//!  * MySQL: `driver::mysql::types`
//!  * SQLite: `driver::sqlite::types`
//!
//! Any external types that have had [`Type`] implemented for, are re-exported in this module
//! for convenience as downstream users need to use a compatible version of the external crate
//! to take advantage of the implementation.
//!
//! # Nullable
//!
//! To represent nullable SQL types, `Option<T>` is supported where `T` implements `Type`.
//! An `Option<T>` represents a potentially `NULL` value from SQL.

use std::{borrow::Cow, rc::Rc, sync::Arc};

use crate::database::Database;
use crate::type_info::TypeInfo;

mod non_zero;

mod json;

mod text;

#[doc(no_inline)]
pub use uuid::{self, Uuid};

pub mod chrono {
    #[doc(no_inline)]
    pub use chrono::{
        DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
    };
}

#[doc(no_inline)]
pub use rust_decimal::Decimal;

pub use json::{Json, JsonRawValue, JsonValue};
pub use text::Text;

/// Indicates that a SQL type is supported for a database.
///
/// ## Manual implementation
///
/// Implement this trait for user-defined types to specify their SQL type. A transparent
/// wrapper can delegate to its inner type:
///
/// ```rust
/// use driver::{Type, postgres::{PgTypeInfo, Postgres}};
///
/// struct UserId(i64);
///
/// impl Type<Postgres> for UserId {
///     fn type_info() -> PgTypeInfo {
///         <i64 as Type<Postgres>>::type_info()
///     }
/// }
/// ```
pub trait Type<DB: Database> {
    /// Returns the canonical SQL type for this Rust type.
    ///
    /// When binding arguments, this is used to tell the database what is about to be sent; which,
    /// the database then uses to guide query plans. This can be overridden by `Encode::produces`.
    ///
    fn type_info() -> DB::TypeInfo;

    /// Determines if this Rust type is compatible with the given SQL type.
    ///
    /// When decoding values from a row, this method is checked to determine if we should continue
    /// or raise a runtime type mismatch error.
    ///
    /// When binding arguments, this method determines whether the Rust type is acceptable.
    ///
    /// Defaults to checking [`TypeInfo::type_compatible()`].
    fn compatible(ty: &DB::TypeInfo) -> bool {
        Self::type_info().type_compatible(ty)
    }
}

// for references, the underlying SQL type is identical
impl<T: ?Sized + Type<DB>, DB: Database> Type<DB> for &'_ T {
    fn type_info() -> DB::TypeInfo {
        <T as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <T as Type<DB>>::compatible(ty)
    }
}

// for optionals, the underlying SQL type is identical
impl<T: Type<DB>, DB: Database> Type<DB> for Option<T> {
    fn type_info() -> DB::TypeInfo {
        <T as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        ty.is_null() || <T as Type<DB>>::compatible(ty)
    }
}

macro_rules! impl_type_for_smartpointer {
    ($smart_pointer:ty) => {
        impl<T, DB: Database> Type<DB> for $smart_pointer
        where
            T: Type<DB> + ?Sized,
        {
            fn type_info() -> DB::TypeInfo {
                <T as Type<DB>>::type_info()
            }

            fn compatible(ty: &DB::TypeInfo) -> bool {
                <T as Type<DB>>::compatible(ty)
            }
        }
    };
}

impl_type_for_smartpointer!(Arc<T>);
impl_type_for_smartpointer!(Box<T>);
impl_type_for_smartpointer!(Rc<T>);

impl<T, DB: Database> Type<DB> for Cow<'_, T>
where
    // `ToOwned` is required here to satisfy `Cow`
    T: Type<DB> + ToOwned + ?Sized,
{
    fn type_info() -> DB::TypeInfo {
        <T as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <T as Type<DB>>::compatible(ty)
    }
}
