use std::fmt::{self, Display, Formatter};

use crate::collation::Collation;
use crate::protocol::text::{ColumnDefinition, ColumnFlags, ColumnType, ExtendedType};
pub(crate) use driver_core::type_info::*;

/// Type information for a MySql type.
#[derive(Debug, Clone)]
pub struct MySqlTypeInfo {
    pub(crate) r#type: ColumnType,
    pub(crate) flags: ColumnFlags,
    pub(crate) collation: Collation,

    // [max_size] for integer types, this is (M) in BIT(M) or TINYINT(M)
    pub(crate) max_size: Option<u32>,

    pub(crate) extended: ExtendedType,
}

impl MySqlTypeInfo {
    pub(crate) const fn binary(ty: ColumnType) -> Self {
        Self {
            r#type: ty,
            flags: ColumnFlags::BINARY,
            collation: Collation::BINARY,
            max_size: None,
            extended: ExtendedType::None,
        }
    }

    pub(crate) fn from_column(column: &ColumnDefinition) -> Self {
        Self {
            r#type: column.r#type,
            flags: column.flags,
            collation: column.collation,
            max_size: Some(column.max_size),
            extended: column.extended,
        }
    }
}

impl Display for MySqlTypeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

impl TypeInfo for MySqlTypeInfo {
    fn is_null(&self) -> bool {
        matches!(self.r#type, ColumnType::Null)
    }

    fn name(&self) -> &str {
        match self.extended {
            ExtendedType::Json => "JSON",
            ExtendedType::Uuid => "UUID",
            ExtendedType::None => self.r#type.name(self.flags, self.collation, self.max_size),
        }
    }
}

impl PartialEq<MySqlTypeInfo> for MySqlTypeInfo {
    fn eq(&self, other: &MySqlTypeInfo) -> bool {
        if self.r#type != other.r#type {
            return false;
        }

        match self.r#type {
            ColumnType::Tiny
            | ColumnType::Short
            | ColumnType::Long
            | ColumnType::Int24
            | ColumnType::LongLong => {
                return self.flags.contains(ColumnFlags::UNSIGNED)
                    == other.flags.contains(ColumnFlags::UNSIGNED);
            }

            // for string types, check that our charset matches
            ColumnType::VarChar
            | ColumnType::Blob
            | ColumnType::TinyBlob
            | ColumnType::MediumBlob
            | ColumnType::LongBlob
            | ColumnType::String
            | ColumnType::VarString
            | ColumnType::Enum => {
                return self.flags == other.flags;
            }
            _ => {}
        }

        true
    }
}

impl Eq for MySqlTypeInfo {}
