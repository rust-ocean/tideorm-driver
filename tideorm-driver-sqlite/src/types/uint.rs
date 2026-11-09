use crate::arguments::SqliteArgumentsBuffer;
use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::types::Type;
use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

impl Type<Sqlite> for u8 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Int4)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Int4 | DataType::Integer)
    }
}

impl Encode<'_, Sqlite> for u8 {
    fn encode_by_ref(&self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int(*self as i32));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for u8 {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        // Use `sqlite3_value_int64()` because `sqlite3_value_int()` silently truncates.
        Ok(value.int64()?.try_into()?)
    }
}

impl Type<Sqlite> for u16 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Int4)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Int4 | DataType::Integer)
    }
}

impl Encode<'_, Sqlite> for u16 {
    fn encode_by_ref(&self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int(*self as i32));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for u16 {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64()?.try_into()?)
    }
}

impl Type<Sqlite> for u32 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Integer)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Int4 | DataType::Integer)
    }
}

impl Encode<'_, Sqlite> for u32 {
    fn encode_by_ref(&self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int64(*self as i64));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for u32 {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64()?.try_into()?)
    }
}

impl Type<Sqlite> for u64 {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Integer)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Int4 | DataType::Integer)
    }
}

/// SQLite's integers are signed: a `u64` past `i64::MAX` is refused.
impl Encode<'_, Sqlite> for u64 {
    fn encode_by_ref(&self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Int64(i64::try_from(*self)?));

        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Sqlite> for u64 {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64()?.try_into()?)
    }
}
