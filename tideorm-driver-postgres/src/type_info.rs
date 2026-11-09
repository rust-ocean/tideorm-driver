use std::fmt::{self, Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;

use crate::ext::ustr::UStr;
use crate::types::Oid;

pub(crate) use driver_core::type_info::TypeInfo;

mod constants;
mod pg_type;
mod pg_type_kind;
mod pg_type_names;

pub use pg_type::PgType;

/// Type information for a PostgreSQL type.
///
/// ### Note: Implementation of `==` ([`PartialEq::eq()`])
/// Because `==` on [`TypeInfo`]s has been used throughout the driver API as a synonym for
/// type compatibility, e.g. in the default impl of
/// [`Type::compatible()`][driver_core::types::Type::compatible],
/// some concessions have been made in the implementation.
///
/// When comparing two `PgTypeInfo`s using the `==` operator ([`PartialEq::eq()`]),
/// if one was constructed with [`Self::with_oid()`] and the other with [`Self::with_name()`] or
/// [`Self::array_of()`], `==` will return `true`:
///
/// ```
/// # use driver::postgres::{types::Oid, PgTypeInfo};
/// // Potentially surprising result, this assert will pass:
/// assert_eq!(PgTypeInfo::with_oid(Oid(1)), PgTypeInfo::with_name("definitely_not_real"));
/// ```
///
/// Since it is not possible in this case to prove the types are _not_ compatible (because
/// both `PgTypeInfo`s need to be resolved by an active connection to know for sure)
/// and type compatibility is mainly done as a sanity check anyway,
/// it was deemed acceptable to fudge equality in this very specific case.
///
/// This also applies when querying with the text protocol (a query with no arguments, which is
/// not prepared), as the connection will be unable
/// to look up the type info like it normally does when preparing a statement: it won't know
/// what the OIDs of the output columns will be until it's in the middle of reading the result,
/// and by that time it's too late.
///
/// To compare types for exact equality, use [`Self::type_eq()`] instead.
#[derive(Debug, Clone, PartialEq)]
pub struct PgTypeInfo(pub(crate) PgType);

impl Deref for PgTypeInfo {
    type Target = PgType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct PgCustomType {
    pub(crate) oid: Oid,
    pub(crate) name: UStr,
    pub(crate) kind: PgTypeKind,
}

#[derive(Debug, Clone)]
pub enum PgTypeKind {
    Simple,
    Pseudo,
    Domain(PgTypeInfo),
    Composite(Arc<[(String, PgTypeInfo)]>),
    Array(PgTypeInfo),
    Enum(Arc<[String]>),
    Range(PgTypeInfo),
    /// A multirange and the subtype of its range bounds.
    Multirange(PgTypeInfo),
}

#[derive(Debug, Clone)]
pub struct PgArrayOf {
    pub(crate) elem_name: UStr,
    pub(crate) name: Box<str>,
}

impl PgTypeInfo {
    /// Returns the corresponding `PgTypeInfo` if the OID is a built-in
    /// type recognized by the driver.
    pub(crate) fn try_from_oid(oid: Oid) -> Option<Self> {
        PgType::try_from_oid(oid).map(Self)
    }

    /// Returns the _kind_ (simple, array, enum, etc.) for this type.
    pub fn kind(&self) -> &PgTypeKind {
        self.0.kind()
    }

    /// Returns the OID for this type, if available.
    ///
    /// The OID may not be available if the driver only knows the type by name.
    /// It will have to be resolved by a `PgConnection` at runtime which
    /// will yield a new and semantically distinct `TypeInfo` instance.
    ///
    /// This method does not perform any such lookup.
    ///
    /// ### Note
    /// With the exception of [the default `pg_type` catalog][pg_type], type OIDs are *not* stable in PostgreSQL.
    /// If a type is added by an extension, its OID will be assigned when the `CREATE EXTENSION` statement is executed,
    /// and so can change depending on what extensions are installed and in what order, as well as the exact
    /// version of PostgreSQL.
    ///
    /// [pg_type]: https://github.com/postgres/postgres/blob/master/src/include/catalog/pg_type.dat
    pub fn oid(&self) -> Option<Oid> {
        self.0.try_oid()
    }

    /// Create a `PgTypeInfo` from a type name.
    ///
    /// The OID for the type will be fetched from Postgres on use of
    /// a value of this type. The fetched OID will be cached per-connection.
    ///
    /// ### Note: Type Names Prefixed with `_`
    /// In `pg_catalog.pg_type`, Postgres prefixes a type name with `_` to denote an array of that
    /// type, e.g. `int4[]` actually exists in `pg_type` as `_int4`.
    ///
    /// Previously, it was necessary in manual [`PgHasArrayType`][crate::PgHasArrayType] impls
    /// to return [`PgTypeInfo::with_name()`] with the type name prefixed with `_` to denote
    /// an array type, but this would not work with schema-qualified names.
    ///
    /// As of 0.8, [`PgTypeInfo::array_of()`] is used to declare an array type,
    /// and the Postgres driver is now able to properly resolve arrays of custom types,
    /// even in other schemas, which was not previously supported.
    ///
    /// It is highly recommended to migrate existing usages to [`PgTypeInfo::array_of()`] where
    /// applicable.
    ///
    /// However, to maintain compatibility, the driver now infers any type name prefixed with `_`
    /// to be an array of that type. This may introduce some breakages for types which use
    /// a `_` prefix but which are not arrays.
    ///
    /// As a workaround, type names with `_` as a prefix but which are not arrays should be wrapped
    /// in quotes, e.g.:
    /// ```rust
    /// use driver::postgres::{PgTypeInfo, Postgres};
    /// use driver::{Type, TypeInfo};
    ///
    /// // The quotes prevent the driver from inferring `_foo` as an array type.
    /// struct Foo;
    ///
    /// impl Type<Postgres> for Foo {
    ///     fn type_info() -> PgTypeInfo {
    ///         PgTypeInfo::with_name(r#""_foo""#)
    ///     }
    /// }
    ///
    /// assert_eq!(Foo::type_info().name(), r#""_foo""#);
    /// ```
    pub const fn with_name(name: &'static str) -> Self {
        Self(PgType::DeclareWithName(UStr::Static(name)))
    }

    /// Create a `PgTypeInfo` of an array from the name of its element type.
    ///
    /// The array type OID will be fetched from Postgres on use of a value of this type.
    /// The fetched OID will be cached per-connection.
    pub fn array_of(elem_name: &'static str) -> Self {
        // to satisfy `name()` and `display_name()`, we need to construct strings to return
        Self(PgType::DeclareArrayOf(Arc::new(PgArrayOf {
            elem_name: elem_name.into(),
            name: format!("{elem_name}[]").into(),
        })))
    }

    /// Create a `PgTypeInfo` from an OID.
    ///
    /// Note that the OID for a type is very dependent on the environment. If you only ever use
    /// one database or if this is an unhandled built-in type, you should be fine. Otherwise,
    /// you will be better served using [`Self::with_name()`].
    ///
    /// ### Note: Interaction with `==`
    /// This constructor may give surprising results with `==`.
    ///
    /// See [the type-level docs][Self] for details.
    pub const fn with_oid(oid: Oid) -> Self {
        Self(PgType::DeclareWithOid(oid))
    }

    /// Returns `true` if `self` can be compared exactly to `other`.
    ///
    /// Unlike `==`, this returns `false` when either type is only declared by
    /// name or OID and not yet resolved.
    pub fn type_eq(&self, other: &Self) -> bool {
        self.eq_impl(other, false)
    }

    /// The name of the date, time or timestamp type under this type's domains.
    pub(crate) fn moment_name(&self) -> Option<&'static str> {
        let name = self.base_type().display_name();
        ["DATE", "TIME", "TIMESTAMP", "TIMESTAMPTZ"]
            .into_iter()
            .find(|moment| *moment == name)
    }

    /// The type's kind, or `None` for a type declared by name or OID and not
    /// yet resolved, which has none to tell.
    pub(crate) fn resolved_kind(&self) -> Option<&PgTypeKind> {
        match &self.0 {
            PgType::DeclareWithOid(_) | PgType::DeclareWithName(_) | PgType::DeclareArrayOf(_) => {
                None
            }
            _ => Some(self.kind()),
        }
    }

    /// The type a domain is over, through every domain between; the type
    /// itself for anything else.
    pub(crate) fn base_type(&self) -> &PgTypeInfo {
        let mut ty = self;
        while let Some(PgTypeKind::Domain(base)) = ty.resolved_kind() {
            ty = base;
        }
        ty
    }
}

impl TypeInfo for PgTypeInfo {
    fn name(&self) -> &str {
        self.0.display_name()
    }

    fn is_null(&self) -> bool {
        false
    }

    fn type_compatible(&self, other: &Self) -> bool
    where
        Self: Sized,
    {
        self == other
    }
}

impl PartialEq<PgCustomType> for PgCustomType {
    fn eq(&self, other: &PgCustomType) -> bool {
        other.oid == self.oid
    }
}

impl Display for PgTypeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

impl PartialEq<PgType> for PgType {
    fn eq(&self, other: &PgType) -> bool {
        self.eq_impl(other, true)
    }
}

/// Check type names for equality, respecting Postgres' case sensitivity rules for identifiers.
///
/// https://www.postgresql.org/docs/current/sql-syntax-lexical.html#SQL-SYNTAX-IDENTIFIERS
fn name_eq(name1: &str, name2: &str) -> bool {
    // Cop-out of processing Unicode escapes by just using string equality.
    if name1.starts_with("U&") {
        // If `name2` doesn't start with `U&` this will automatically be `false`.
        return name1 == name2;
    }

    let mut chars1 = identifier_chars(name1);
    let mut chars2 = identifier_chars(name2);

    while let (Some(a), Some(b)) = (chars1.next(), chars2.next()) {
        if !a.eq(&b) {
            return false;
        }
    }

    chars1.next().is_none() && chars2.next().is_none()
}

struct IdentifierChar {
    ch: char,
    case_sensitive: bool,
}

impl IdentifierChar {
    fn eq(&self, other: &Self) -> bool {
        if self.case_sensitive || other.case_sensitive {
            self.ch == other.ch
        } else {
            self.ch.eq_ignore_ascii_case(&other.ch)
        }
    }
}

/// Return an iterator over all significant characters of an identifier.
///
/// Ignores non-escaped quotation marks.
fn identifier_chars(ident: &str) -> impl Iterator<Item = IdentifierChar> + '_ {
    let mut case_sensitive = false;
    let mut last_char_quote = false;

    ident.chars().filter_map(move |ch| {
        if ch == '"' {
            if last_char_quote {
                last_char_quote = false;
            } else {
                last_char_quote = true;
                return None;
            }
        } else if last_char_quote {
            last_char_quote = false;
            case_sensitive = !case_sensitive;
        }

        Some(IdentifierChar { ch, case_sensitive })
    })
}

#[cfg(test)]
#[path = "../tests/unit/type_info_tests.rs"]
mod tests;
