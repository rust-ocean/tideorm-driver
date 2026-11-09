use serde::{Deserialize, Serialize};

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::protocol::text::ColumnType;
use crate::types::{Json, Type};
use crate::{MySql, MySqlTypeInfo, MySqlValueRef};

impl<T> Type<MySql> for Json<T> {
    fn type_info() -> MySqlTypeInfo {
        // MySql uses the `CHAR` type to pass JSON data from and to the client
        // NOTE: This is forwards-compatible with MySQL v8+ as CHAR is a common transmission format
        //       and has nothing to do with the native storage ability of MySQL v8+
        MySqlTypeInfo::binary(ColumnType::String)
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        ty.r#type == ColumnType::Json
            || <&str as Type<MySql>>::compatible(ty)
            || <&[u8] as Type<MySql>>::compatible(ty)
    }
}

impl<T> Encode<'_, MySql> for Json<T>
where
    T: Serialize,
{
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> Result<IsNull, BoxDynError> {
        // A length-prefixed string whose length always takes the 8-byte form, reserved up
        // front, so the JSON serializes straight into the buffer.
        let lenenc_start = buf.len();

        buf.extend_from_slice(&[0u8; 9]);

        let encode_start = buf.len();
        self.encode_to(buf)?;
        let encoded_len = (buf.len() - encode_start) as u64;

        // This prefix indicates that the following 8 bytes are a little-endian integer.
        buf[lenenc_start] = 0xFE;
        buf[lenenc_start + 1..][..8].copy_from_slice(&encoded_len.to_le_bytes());

        Ok(IsNull::No)
    }
}

impl<'r, T> Decode<'r, MySql> for Json<T>
where
    T: 'r + Deserialize<'r>,
{
    fn decode(value: MySqlValueRef<'r>) -> Result<Self, BoxDynError> {
        Json::decode_from_string(value.as_str()?)
    }
}
