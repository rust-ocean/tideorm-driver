use std::{
    collections::{btree_map, BTreeMap},
    mem,
    ops::{Deref, DerefMut},
    str,
};

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::BoxDynError,
    types::Type,
    PgArgumentBuffer, PgHasArrayType, PgTypeInfo, PgValueRef, Postgres,
};
use driver_core::bytes::Buf;

/// Key-value support (`hstore`) for Postgres.
///
/// The driver maps `hstore` to a `BTreeMap<String, Option<String>>`.
///
/// See [the Postgres manual, Appendix F, Section 18][PG.F.18]
///
/// [PG.F.18]: https://www.postgresql.org/docs/current/hstore.html
///
/// ### Note: Requires Postgres 8.3+
/// Introduced as a method for storing unstructured data, the `hstore` extension was first added in
/// Postgres 8.3.
///
/// ### Note: Extension Required
/// The `hstore` extension is not enabled by default in Postgres. You will need to do so explicitly:
///
/// ```sql
/// CREATE EXTENSION IF NOT EXISTS hstore;
/// ```
///
/// # Examples
///
/// ```
/// # use driver::postgres::types::PgHstore;
/// // Shows basic usage of the PgHstore type.
/// //
/// #[derive(Clone, Debug, Default, Eq, PartialEq)]
/// struct UserCreate<'a> {
///     username: &'a str,
///     password: &'a str,
///     additional_data: PgHstore
/// }
///
/// let mut new_user = UserCreate {
///     username: "name.surname@email.com",
///     password: "@super_secret_1",
///     ..Default::default()
/// };
///
/// new_user.additional_data.insert("department".to_string(), Some("IT".to_string()));
/// new_user.additional_data.insert("equipment_issued".to_string(), None);
/// ```
/// ```rust,no_run
/// # async fn example(pg_conn: &mut driver::postgres::PgConnection) -> driver::Result<()> {
/// # use driver::postgres::types::PgHstore;
/// # let new_user = ("name", "password", PgHstore::default());
/// # use driver::Row;
/// let id: i64 = driver::query(
///     "insert into users(username, password, additional_data) values($1, $2, $3) returning id"
/// )
/// .bind(new_user.0)
/// .bind(new_user.1)
/// .bind(new_user.2)
/// .fetch_one(pg_conn)
/// .await?
/// .try_get(0)?;
/// # Ok(())
/// # }
/// ```
///
/// ```
/// # use driver::postgres::types::PgHstore;
/// // PgHstore implements FromIterator to simplify construction.
/// //
/// let additional_data = PgHstore::from_iter([
///     ("department".to_string(), Some("IT".to_string())),
///     ("equipment_issued".to_string(), None),
/// ]);
///
/// assert_eq!(additional_data["department"], Some("IT".to_string()));
/// assert_eq!(additional_data["equipment_issued"], None);
///
/// // Also IntoIterator for ease of iteration.
/// //
/// for (key, value) in additional_data {
///     println!("{key}: {value:?}");
/// }
/// ```
///
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PgHstore(pub BTreeMap<String, Option<String>>);

impl Deref for PgHstore {
    type Target = BTreeMap<String, Option<String>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PgHstore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromIterator<(String, String)> for PgHstore {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        iter.into_iter().map(|(k, v)| (k, Some(v))).collect()
    }
}

impl FromIterator<(String, Option<String>)> for PgHstore {
    fn from_iter<T: IntoIterator<Item = (String, Option<String>)>>(iter: T) -> Self {
        let mut result = Self::default();

        for (key, value) in iter {
            result.0.insert(key, value);
        }

        result
    }
}

impl IntoIterator for PgHstore {
    type Item = (String, Option<String>);
    type IntoIter = btree_map::IntoIter<String, Option<String>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Type<Postgres> for PgHstore {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("hstore")
    }
}

impl PgHasArrayType for PgHstore {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::array_of("hstore")
    }
}

impl<'r> Decode<'r, Postgres> for PgHstore {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let mut buf = <&[u8] as Decode<Postgres>>::decode(value)?;
        let len = read_length(&mut buf)?;

        let len =
            usize::try_from(len).map_err(|_| format!("PgHstore: length out of range: {len}"))?;

        let mut result = Self::default();

        for i in 0..len {
            let key = read_string(&mut buf)
                .map_err(|e| format!("PgHstore: error reading {i}th key: {e}"))?
                .ok_or_else(|| format!("PgHstore: expected {i}th key, got nothing"))?;

            let value = read_string(&mut buf)
                .map_err(|e| format!("PgHstore: error reading value for key {key:?}: {e}"))?;

            result.insert(key, value);
        }

        if !buf.is_empty() {
            tracing::warn!("{} unread bytes at the end of HSTORE value", buf.len());
        }

        Ok(result)
    }
}

impl Encode<'_, Postgres> for PgHstore {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend_from_slice(&i32::to_be_bytes(
            self.0
                .len()
                .try_into()
                .map_err(|_| format!("PgHstore length out of range: {}", self.0.len()))?,
        ));

        for (i, (key, val)) in self.0.iter().enumerate() {
            let key_bytes = key.as_bytes();

            let key_len = i32::try_from(key_bytes.len()).map_err(|_| {
                // Doesn't make sense to print the key itself: it's more than 2 GiB long!
                format!(
                    "PgHstore: length of {i}th key out of range: {} bytes",
                    key_bytes.len()
                )
            })?;

            buf.extend_from_slice(&i32::to_be_bytes(key_len));
            buf.extend_from_slice(key_bytes);

            match val {
                Some(val) => {
                    let val_bytes = val.as_bytes();

                    let val_len = i32::try_from(val_bytes.len()).map_err(|_| {
                        format!(
                            "PgHstore: value length for key {key:?} out of range: {} bytes",
                            val_bytes.len()
                        )
                    })?;
                    buf.extend_from_slice(&i32::to_be_bytes(val_len));
                    buf.extend_from_slice(val_bytes);
                }
                None => {
                    buf.extend_from_slice(&i32::to_be_bytes(-1));
                }
            }
        }

        Ok(IsNull::No)
    }
}

fn read_length(buf: &mut &[u8]) -> Result<i32, String> {
    if buf.len() < mem::size_of::<i32>() {
        return Err(format!(
            "expected {} bytes, got {}",
            mem::size_of::<i32>(),
            buf.len()
        ));
    }

    Ok(buf.get_i32())
}

fn read_string(buf: &mut &[u8]) -> Result<Option<String>, String> {
    let len = read_length(buf)?;

    match len {
        -1 => Ok(None),
        len => {
            let len =
                usize::try_from(len).map_err(|_| format!("string length out of range: {len}"))?;

            if buf.len() < len {
                return Err(format!("expected {len} bytes, got {}", buf.len()));
            }

            let (val, rest) = buf.split_at(len);
            *buf = rest;

            Ok(Some(
                str::from_utf8(val).map_err(|e| e.to_string())?.to_string(),
            ))
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/types_hstore_tests.rs"]
mod test;
