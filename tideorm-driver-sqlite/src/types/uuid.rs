use crate::arguments::SqliteArgumentsBuffer;
use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::type_info::DataType;
use crate::types::Type;
use crate::value::ValueRef;
use crate::{Sqlite, SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use std::sync::Arc;
use uuid::{
    fmt::{Hyphenated, Simple},
    Uuid,
};

impl Type<Sqlite> for Uuid {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Blob)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Blob | DataType::Text)
    }
}

impl Encode<'_, Sqlite> for Uuid {
    fn encode_by_ref(&self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Blob(Arc::new(
            self.as_bytes().to_vec(),
        )));

        Ok(IsNull::No)
    }
}

impl Decode<'_, Sqlite> for Uuid {
    /// A blob holds the UUID's bytes, as this driver binds one; text, as a
    /// column default, raw SQL or another tool writes one, holds its text.
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        if value.type_info().0 == DataType::Text {
            Uuid::try_parse(value.text_borrowed()?).map_err(Into::into)
        } else {
            Uuid::from_slice(value.blob_borrowed()).map_err(Into::into)
        }
    }
}

impl Type<Sqlite> for Hyphenated {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Text)
    }
}

impl Encode<'_, Sqlite> for Hyphenated {
    fn encode_by_ref(&self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Arc::new(self.to_string())));

        Ok(IsNull::No)
    }
}

impl Decode<'_, Sqlite> for Hyphenated {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let uuid: Result<Uuid, BoxDynError> =
            Uuid::parse_str(&value.text_borrowed().map(ToOwned::to_owned)?).map_err(Into::into);

        Ok(uuid?.hyphenated())
    }
}

impl Type<Sqlite> for Simple {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Text)
    }
}

impl Encode<'_, Sqlite> for Simple {
    fn encode_by_ref(&self, args: &mut SqliteArgumentsBuffer) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Text(Arc::new(self.to_string())));

        Ok(IsNull::No)
    }
}

impl Decode<'_, Sqlite> for Simple {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let uuid: Result<Uuid, BoxDynError> =
            Uuid::parse_str(&value.text_borrowed().map(ToOwned::to_owned)?).map_err(Into::into);

        Ok(uuid?.simple())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/uuid_tests.rs"]
mod tests;
