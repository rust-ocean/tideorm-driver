//! What a backend reports about a SQL type.

use std::fmt::{Debug, Display};

/// A backend's description of a SQL type: of a parameter, a column or a value.
pub trait TypeInfo: Debug + Display + Clone + PartialEq<Self> + Send + Sync {
    /// Whether this is the type reported for a `NULL` value, which carries no type of
    /// its own.
    fn is_null(&self) -> bool;

    /// The type's name in the backend's own SQL, upper case and without length or
    /// precision: `TEXT`, `INT`, `VARCHAR`.
    fn name(&self) -> &str;

    /// Whether `self` and `other` are compatible types. The default asks for equal
    /// types; a backend widens it where its types interconvert.
    fn type_compatible(&self, other: &Self) -> bool
    where
        Self: Sized,
    {
        self == other
    }
}
