//! The values a statement's parameters are bound to.

use crate::database::Database;
use crate::encode::Encode;
use crate::error::BoxDynError;
use crate::types::Type;

/// The values for a statement's parameters, in parameter order, encoded into the
/// backend's own buffer.
// `len` reports progress while binding; this is no collection to ask `is_empty` of.
#[allow(clippy::len_without_is_empty)]
pub trait Arguments: Send + Sized + Default {
    type Database: Database;

    /// Makes room for `additional` more values taking `size` bytes in all, so adding
    /// them does not reallocate.
    fn reserve(&mut self, additional: usize, size: usize);

    /// Encodes `value` as the next parameter's value.
    fn add<'t, T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'t, Self::Database> + Type<Self::Database>;

    /// How many values have been added so far.
    fn len(&self) -> usize;
}

/// A value a query accepts as its arguments, turned into the backend's `Arguments`.
pub trait IntoArguments<DB: Database>: Sized + Send {
    fn into_arguments(self) -> <DB as Database>::Arguments;
}

/// Implements [`IntoArguments`] for a backend's own `Arguments` type, as the identity.
///
/// One blanket impl over every `Arguments` type would need the compiler to prove that
/// `<A::Database as Database>::Arguments` is `A` itself, which it cannot do for a generic
/// `A`; each backend invokes this for its concrete type instead.
#[macro_export]
macro_rules! impl_into_arguments_for_arguments {
    ($Arguments:path) => {
        impl
            $crate::arguments::IntoArguments<<$Arguments as $crate::arguments::Arguments>::Database>
            for $Arguments
        {
            fn into_arguments(self) -> $Arguments {
                self
            }
        }
    };
}
