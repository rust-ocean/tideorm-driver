use super::int::integer_compatible;
use crate::collation::Collation;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::protocol::text::{ColumnFlags, ColumnType, ExtendedType};
use crate::types::Type;
use crate::{MySql, MySqlTypeInfo};

fn uint_type_info(ty: ColumnType) -> MySqlTypeInfo {
    MySqlTypeInfo {
        r#type: ty,
        flags: ColumnFlags::BINARY | ColumnFlags::UNSIGNED,
        collation: Collation::BINARY,
        max_size: None,
        extended: ExtendedType::None,
    }
}

impl Type<MySql> for u8 {
    fn type_info() -> MySqlTypeInfo {
        uint_type_info(ColumnType::Tiny)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl Type<MySql> for u16 {
    fn type_info() -> MySqlTypeInfo {
        uint_type_info(ColumnType::Short)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl Type<MySql> for u32 {
    fn type_info() -> MySqlTypeInfo {
        uint_type_info(ColumnType::Long)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl Type<MySql> for u64 {
    fn type_info() -> MySqlTypeInfo {
        uint_type_info(ColumnType::LongLong)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        integer_compatible(ty)
    }
}

impl Encode<'_, MySql> for u8 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_le_bytes());

        Ok(IsNull::No)
    }
}

impl Encode<'_, MySql> for u16 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_le_bytes());

        Ok(IsNull::No)
    }
}

impl Encode<'_, MySql> for u32 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_le_bytes());

        Ok(IsNull::No)
    }
}

impl Encode<'_, MySql> for u64 {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        buf.extend(&self.to_le_bytes());

        Ok(IsNull::No)
    }
}
