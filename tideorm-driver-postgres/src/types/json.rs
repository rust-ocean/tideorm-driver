//! `JSON` and `JSONB` documents: [`Json<T>`] for any serde type, and `serde_json`'s `Value` and
//! `RawValue` for untyped ones (their `Type`, `Encode` and `Decode` impls go through `Json`).
//!
//! All of them bind as `JSONB`, the type PostgreSQL recommends for stored documents
//! (<https://www.postgresql.org/docs/current/datatype-json.html>), and read either type.

use driver_core::types::Json;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue as JsonRawValue;
use serde_json::Value as JsonValue;

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

/// The first byte of a binary `JSONB` value: its format version, 1 in every PostgreSQL release.
const JSONB_VERSION: u8 = 1;

impl<T> Type<Postgres> for Json<T> {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::JSONB
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        *ty == PgTypeInfo::JSON || *ty == PgTypeInfo::JSONB
    }
}

array_type!(JSONB_ARRAY of Self: [T] Json<T>, [] JsonValue, [] JsonRawValue);

impl<T> Encode<'_, Postgres> for Json<T>
where
    T: Serialize,
{
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        // Binary JSONB is the version byte and then the document's text; binary JSON is the text
        // alone. Which one the parameter is, is known only once the statement is prepared, so
        // the version byte is written now and turned into a space, which JSON ignores, for JSON.
        buf.patch_with(|bytes, ty: &PgTypeInfo| {
            if *ty == PgTypeInfo::JSON || *ty == PgTypeInfo::JSON_ARRAY {
                bytes[0] = b' ';
            }
        });
        buf.push(JSONB_VERSION);
        serde_json::to_writer(&mut **buf, &self.0)?;

        Ok(IsNull::No)
    }
}

impl<'r, T: 'r> Decode<'r, Postgres> for Json<T>
where
    T: Deserialize<'r>,
{
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let mut text = value.as_bytes()?;

        if value.format() == PgValueFormat::Binary && value.type_info == PgTypeInfo::JSONB {
            if text[0] != JSONB_VERSION {
                return Err(
                    format!("unsupported JSONB format version {} (expected 1)", text[0]).into(),
                );
            }
            text = &text[1..];
        }

        Ok(Json(serde_json::from_slice(text)?))
    }
}
