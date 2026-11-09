//! The kind of a `PgType`, its array element, and type equality.

use std::borrow::Cow;

use crate::ext::ustr::UStr;

use super::{name_eq, PgType, PgTypeInfo, PgTypeKind};
use crate::types::Oid;

impl PgType {
    pub(crate) fn kind(&self) -> &PgTypeKind {
        match self {
            PgType::Bool
            | PgType::Bytea
            | PgType::Char
            | PgType::Name
            | PgType::Int8
            | PgType::Int2
            | PgType::Int4
            | PgType::Text
            | PgType::Oid
            | PgType::Json
            | PgType::Point
            | PgType::Lseg
            | PgType::Path
            | PgType::Box
            | PgType::Polygon
            | PgType::Line
            | PgType::Cidr
            | PgType::Float4
            | PgType::Float8
            | PgType::Unknown
            | PgType::Circle
            | PgType::Macaddr8
            | PgType::Macaddr
            | PgType::Inet
            | PgType::Bpchar
            | PgType::Varchar
            | PgType::Date
            | PgType::Time
            | PgType::Timestamp
            | PgType::Timestamptz
            | PgType::Interval
            | PgType::Timetz
            | PgType::Bit
            | PgType::Varbit
            | PgType::Numeric
            | PgType::Record
            | PgType::Uuid
            | PgType::Jsonb
            | PgType::Jsonpath
            | PgType::Money => &PgTypeKind::Simple,
            PgType::JsonArray => &PgTypeKind::Array(PgTypeInfo(PgType::Json)),
            PgType::LineArray => &PgTypeKind::Array(PgTypeInfo(PgType::Line)),
            PgType::CidrArray => &PgTypeKind::Array(PgTypeInfo(PgType::Cidr)),
            PgType::CircleArray => &PgTypeKind::Array(PgTypeInfo(PgType::Circle)),
            PgType::Macaddr8Array => &PgTypeKind::Array(PgTypeInfo(PgType::Macaddr8)),
            PgType::BoolArray => &PgTypeKind::Array(PgTypeInfo(PgType::Bool)),
            PgType::ByteaArray => &PgTypeKind::Array(PgTypeInfo(PgType::Bytea)),
            PgType::CharArray => &PgTypeKind::Array(PgTypeInfo(PgType::Char)),
            PgType::NameArray => &PgTypeKind::Array(PgTypeInfo(PgType::Name)),
            PgType::Int2Array => &PgTypeKind::Array(PgTypeInfo(PgType::Int2)),
            PgType::Int4Array => &PgTypeKind::Array(PgTypeInfo(PgType::Int4)),
            PgType::TextArray => &PgTypeKind::Array(PgTypeInfo(PgType::Text)),
            PgType::BpcharArray => &PgTypeKind::Array(PgTypeInfo(PgType::Bpchar)),
            PgType::VarcharArray => &PgTypeKind::Array(PgTypeInfo(PgType::Varchar)),
            PgType::Int8Array => &PgTypeKind::Array(PgTypeInfo(PgType::Int8)),
            PgType::PointArray => &PgTypeKind::Array(PgTypeInfo(PgType::Point)),
            PgType::LsegArray => &PgTypeKind::Array(PgTypeInfo(PgType::Lseg)),
            PgType::PathArray => &PgTypeKind::Array(PgTypeInfo(PgType::Path)),
            PgType::BoxArray => &PgTypeKind::Array(PgTypeInfo(PgType::Box)),
            PgType::Float4Array => &PgTypeKind::Array(PgTypeInfo(PgType::Float4)),
            PgType::Float8Array => &PgTypeKind::Array(PgTypeInfo(PgType::Float8)),
            PgType::PolygonArray => &PgTypeKind::Array(PgTypeInfo(PgType::Polygon)),
            PgType::OidArray => &PgTypeKind::Array(PgTypeInfo(PgType::Oid)),
            PgType::MacaddrArray => &PgTypeKind::Array(PgTypeInfo(PgType::Macaddr)),
            PgType::InetArray => &PgTypeKind::Array(PgTypeInfo(PgType::Inet)),
            PgType::TimestampArray => &PgTypeKind::Array(PgTypeInfo(PgType::Timestamp)),
            PgType::DateArray => &PgTypeKind::Array(PgTypeInfo(PgType::Date)),
            PgType::TimeArray => &PgTypeKind::Array(PgTypeInfo(PgType::Time)),
            PgType::TimestamptzArray => &PgTypeKind::Array(PgTypeInfo(PgType::Timestamptz)),
            PgType::IntervalArray => &PgTypeKind::Array(PgTypeInfo(PgType::Interval)),
            PgType::NumericArray => &PgTypeKind::Array(PgTypeInfo(PgType::Numeric)),
            PgType::TimetzArray => &PgTypeKind::Array(PgTypeInfo(PgType::Timetz)),
            PgType::BitArray => &PgTypeKind::Array(PgTypeInfo(PgType::Bit)),
            PgType::VarbitArray => &PgTypeKind::Array(PgTypeInfo(PgType::Varbit)),
            PgType::RecordArray => &PgTypeKind::Array(PgTypeInfo(PgType::Record)),
            PgType::UuidArray => &PgTypeKind::Array(PgTypeInfo(PgType::Uuid)),
            PgType::JsonbArray => &PgTypeKind::Array(PgTypeInfo(PgType::Jsonb)),
            PgType::Int4Range => &PgTypeKind::Range(PgTypeInfo::INT4),
            PgType::Int4RangeArray => &PgTypeKind::Array(PgTypeInfo(PgType::Int4Range)),
            PgType::NumRange => &PgTypeKind::Range(PgTypeInfo::NUMERIC),
            PgType::NumRangeArray => &PgTypeKind::Array(PgTypeInfo(PgType::NumRange)),
            PgType::TsRange => &PgTypeKind::Range(PgTypeInfo::TIMESTAMP),
            PgType::TsRangeArray => &PgTypeKind::Array(PgTypeInfo(PgType::TsRange)),
            PgType::TstzRange => &PgTypeKind::Range(PgTypeInfo::TIMESTAMPTZ),
            PgType::TstzRangeArray => &PgTypeKind::Array(PgTypeInfo(PgType::TstzRange)),
            PgType::DateRange => &PgTypeKind::Range(PgTypeInfo::DATE),
            PgType::DateRangeArray => &PgTypeKind::Array(PgTypeInfo(PgType::DateRange)),
            PgType::Int8Range => &PgTypeKind::Range(PgTypeInfo::INT8),
            PgType::Int8RangeArray => &PgTypeKind::Array(PgTypeInfo(PgType::Int8Range)),
            PgType::JsonpathArray => &PgTypeKind::Array(PgTypeInfo(PgType::Jsonpath)),
            PgType::MoneyArray => &PgTypeKind::Array(PgTypeInfo(PgType::Money)),

            PgType::Void => &PgTypeKind::Pseudo,

            PgType::Custom(ty) => &ty.kind,

            PgType::DeclareWithOid(oid) => {
                unreachable!("(bug) use of unresolved type declaration [oid={}]", oid.0);
            }
            PgType::DeclareWithName(name) => {
                unreachable!("(bug) use of unresolved type declaration [name={name}]");
            }
            PgType::DeclareArrayOf(array) => {
                unreachable!(
                    "(bug) use of unresolved type declaration [array of={}]",
                    array.elem_name
                );
            }
        }
    }

    /// If `self` is an array type, return the type info for its element.
    pub(crate) fn try_array_element(&self) -> Option<Cow<'_, PgTypeInfo>> {
        // We explicitly match on all the `None` cases to ensure an exhaustive match.
        match self {
            PgType::Bool
            | PgType::Bytea
            | PgType::Char
            | PgType::Name
            | PgType::Int8
            | PgType::Int2
            | PgType::Int4
            | PgType::Text
            | PgType::Oid
            | PgType::Json
            | PgType::Point
            | PgType::Lseg
            | PgType::Path
            | PgType::Box
            | PgType::Polygon
            | PgType::Line
            | PgType::Cidr
            | PgType::Float4
            | PgType::Float8
            | PgType::Circle
            | PgType::Macaddr8
            | PgType::Money
            | PgType::Macaddr
            | PgType::Inet
            | PgType::Bpchar
            | PgType::Varchar
            | PgType::Date
            | PgType::Time
            | PgType::Timestamp
            | PgType::Timestamptz
            | PgType::Interval
            | PgType::Timetz
            | PgType::Bit
            | PgType::Varbit
            | PgType::Numeric
            | PgType::Record
            | PgType::Uuid
            | PgType::Jsonb
            | PgType::Int4Range
            | PgType::NumRange
            | PgType::TsRange
            | PgType::TstzRange
            | PgType::DateRange
            | PgType::Int8Range
            | PgType::Jsonpath
            // There is no `UnknownArray` or `VoidArray`
            | PgType::Unknown
            | PgType::Void => None,
            PgType::BoolArray => Some(Cow::Owned(PgTypeInfo(PgType::Bool))),
            PgType::ByteaArray => Some(Cow::Owned(PgTypeInfo(PgType::Bytea))),
            PgType::CharArray => Some(Cow::Owned(PgTypeInfo(PgType::Char))),
            PgType::NameArray => Some(Cow::Owned(PgTypeInfo(PgType::Name))),
            PgType::Int8Array => Some(Cow::Owned(PgTypeInfo(PgType::Int8))),
            PgType::Int2Array => Some(Cow::Owned(PgTypeInfo(PgType::Int2))),
            PgType::Int4Array => Some(Cow::Owned(PgTypeInfo(PgType::Int4))),
            PgType::TextArray => Some(Cow::Owned(PgTypeInfo(PgType::Text))),
            PgType::OidArray => Some(Cow::Owned(PgTypeInfo(PgType::Oid))),
            PgType::JsonArray => Some(Cow::Owned(PgTypeInfo(PgType::Json))),
            PgType::PointArray => Some(Cow::Owned(PgTypeInfo(PgType::Point))),
            PgType::LsegArray => Some(Cow::Owned(PgTypeInfo(PgType::Lseg))),
            PgType::PathArray => Some(Cow::Owned(PgTypeInfo(PgType::Path))),
            PgType::BoxArray => Some(Cow::Owned(PgTypeInfo(PgType::Box))),
            PgType::PolygonArray => Some(Cow::Owned(PgTypeInfo(PgType::Polygon))),
            PgType::LineArray => Some(Cow::Owned(PgTypeInfo(PgType::Line))),
            PgType::CidrArray => Some(Cow::Owned(PgTypeInfo(PgType::Cidr))),
            PgType::Float4Array => Some(Cow::Owned(PgTypeInfo(PgType::Float4))),
            PgType::Float8Array => Some(Cow::Owned(PgTypeInfo(PgType::Float8))),
            PgType::CircleArray => Some(Cow::Owned(PgTypeInfo(PgType::Circle))),
            PgType::Macaddr8Array => Some(Cow::Owned(PgTypeInfo(PgType::Macaddr8))),
            PgType::MoneyArray => Some(Cow::Owned(PgTypeInfo(PgType::Money))),
            PgType::MacaddrArray => Some(Cow::Owned(PgTypeInfo(PgType::Macaddr))),
            PgType::InetArray => Some(Cow::Owned(PgTypeInfo(PgType::Inet))),
            PgType::BpcharArray => Some(Cow::Owned(PgTypeInfo(PgType::Bpchar))),
            PgType::VarcharArray => Some(Cow::Owned(PgTypeInfo(PgType::Varchar))),
            PgType::DateArray => Some(Cow::Owned(PgTypeInfo(PgType::Date))),
            PgType::TimeArray => Some(Cow::Owned(PgTypeInfo(PgType::Time))),
            PgType::TimestampArray => Some(Cow::Owned(PgTypeInfo(PgType::Timestamp))),
            PgType::TimestamptzArray => Some(Cow::Owned(PgTypeInfo(PgType::Timestamptz))),
            PgType::IntervalArray => Some(Cow::Owned(PgTypeInfo(PgType::Interval))),
            PgType::TimetzArray => Some(Cow::Owned(PgTypeInfo(PgType::Timetz))),
            PgType::BitArray => Some(Cow::Owned(PgTypeInfo(PgType::Bit))),
            PgType::VarbitArray => Some(Cow::Owned(PgTypeInfo(PgType::Varbit))),
            PgType::NumericArray => Some(Cow::Owned(PgTypeInfo(PgType::Numeric))),
            PgType::RecordArray => Some(Cow::Owned(PgTypeInfo(PgType::Record))),
            PgType::UuidArray => Some(Cow::Owned(PgTypeInfo(PgType::Uuid))),
            PgType::JsonbArray => Some(Cow::Owned(PgTypeInfo(PgType::Jsonb))),
            PgType::Int4RangeArray => Some(Cow::Owned(PgTypeInfo(PgType::Int4Range))),
            PgType::NumRangeArray => Some(Cow::Owned(PgTypeInfo(PgType::NumRange))),
            PgType::TsRangeArray => Some(Cow::Owned(PgTypeInfo(PgType::TsRange))),
            PgType::TstzRangeArray => Some(Cow::Owned(PgTypeInfo(PgType::TstzRange))),
            PgType::DateRangeArray => Some(Cow::Owned(PgTypeInfo(PgType::DateRange))),
            PgType::Int8RangeArray => Some(Cow::Owned(PgTypeInfo(PgType::Int8Range))),
            PgType::JsonpathArray => Some(Cow::Owned(PgTypeInfo(PgType::Jsonpath))),

            PgType::Custom(ty) => match &ty.kind {
                PgTypeKind::Simple
                | PgTypeKind::Pseudo
                | PgTypeKind::Domain(_)
                | PgTypeKind::Composite(_)
                | PgTypeKind::Enum(_)
                | PgTypeKind::Range(_)
                | PgTypeKind::Multirange(_) => None,
                PgTypeKind::Array(ref elem_type_info) => Some(Cow::Borrowed(elem_type_info)),
            },
            PgType::DeclareWithOid(_) => None,
            PgType::DeclareWithName(name) => {
                // An array type is named after its element, with a `_` prefix.
                UStr::strip_prefix(name, "_")
                    .map(|elem| Cow::Owned(PgTypeInfo(PgType::DeclareWithName(elem))))
            }
            PgType::DeclareArrayOf(array) => Some(Cow::Owned(PgTypeInfo(PgType::DeclareWithName(
                array.elem_name.clone(),
            )))),
        }
    }

    /// Returns `true` if this type cannot be matched by name.
    pub(super) fn is_declare_with_oid(&self) -> bool {
        matches!(self, Self::DeclareWithOid(_))
    }

    /// Compare two `PgType`s, first by OID, then by array element, then by name.
    ///
    /// If `soft_eq` is true and `self` or `other` is `DeclareWithOid` but not both, return `true`
    /// before checking names.
    pub(super) fn eq_impl(&self, other: &Self, soft_eq: bool) -> bool {
        if let (Some(a), Some(b)) = (self.try_base_oid(), other.try_base_oid()) {
            // If there are OIDs available, use OIDs to perform a direct match
            return a == b;
        }

        if soft_eq && (self.is_declare_with_oid() || other.is_declare_with_oid()) {
            // If we get to this point, one instance is `DeclareWithOid()` and the other is
            // `DeclareArrayOf()` or `DeclareWithName()`, which means we can't compare the two.
            //
            // Since this is only likely to occur when using the text protocol where we can't
            // resolve type names before executing a query, we can just opt out of typechecking.
            return true;
        }

        if let (Some(elem_a), Some(elem_b)) = (self.try_array_element(), other.try_array_element())
        {
            return elem_a == elem_b;
        }

        // Otherwise, perform a match on the name
        name_eq(self.name(), other.name())
    }

    // Tries to return the OID of the type, returns the OID of the base_type for domain types
    #[inline(always)]
    pub(super) fn try_base_oid(&self) -> Option<Oid> {
        match self {
            PgType::Custom(custom) => match &custom.kind {
                PgTypeKind::Domain(domain) => domain.try_oid(),
                _ => Some(custom.oid),
            },
            ty => ty.try_oid(),
        }
    }
}
