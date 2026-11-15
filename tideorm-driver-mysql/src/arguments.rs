use crate::encode::{Encode, IsNull};
use crate::io::MySqlBufMutExt;
use crate::protocol::text::{ColumnFlags, ColumnType};
use crate::types::Type;
use crate::{MySql, MySqlTypeInfo};
pub(crate) use driver_core::arguments::*;
use driver_core::error::BoxDynError;
use std::ops::Deref;

/// Implementation of [`Arguments`] for MySQL.
#[derive(Debug, Default, Clone)]
pub struct MySqlArguments {
    pub(crate) values: Vec<u8>,
    pub(crate) types: Vec<MySqlTypeInfo>,
    pub(crate) null_bitmap: NullBitMap,
    // where each value's bytes end in `values`
    ends: Vec<usize>,
}

impl MySqlArguments {
    pub(crate) fn add<'q, T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'q, MySql> + Type<MySql>,
    {
        let ty = value.produces().unwrap_or_else(T::type_info);

        let value_length_before_encoding = self.values.len();
        let is_null = match value.encode(&mut self.values) {
            Ok(is_null) => is_null,
            Err(error) => {
                // reset the value buffer to its previous value if encoding failed so we don't leave a half-encoded value behind
                self.values.truncate(value_length_before_encoding);
                return Err(error);
            }
        };

        self.types.push(ty);
        self.null_bitmap.push(is_null);
        self.ends.push(self.values.len());

        Ok(())
    }

    /// These arguments with every unsigned integer whose top bit is set sent
    /// signed, or `None` when none is. Vitess reads a parameter as signed
    /// whatever its flag says (a `TINYINT UNSIGNED` 255 as -1), so such a
    /// value up to `i64::MAX` goes as a `BIGINT` and one past it as a
    /// `DECIMAL`'s text, which the server converts exactly.
    pub(crate) fn signed(&self) -> Option<Self> {
        // a value with its top bit clear reads the same either way, and a
        // NULL has no bytes
        let misread = |ty: &MySqlTypeInfo, bytes: &[u8]| {
            ty.flags.contains(ColumnFlags::UNSIGNED)
                && matches!(
                    ty.r#type,
                    ColumnType::Tiny
                        | ColumnType::Short
                        | ColumnType::Int24
                        | ColumnType::Long
                        | ColumnType::LongLong
                )
                && bytes.last().is_some_and(|byte| byte & 0x80 != 0)
        };
        let values = || {
            let starts = std::iter::once(0).chain(self.ends.iter().copied());
            self.types
                .iter()
                .zip(starts.zip(&self.ends))
                .map(|(ty, (start, &end))| (ty, &self.values[start..end]))
        };
        if !values().any(|(ty, bytes)| misread(ty, bytes)) {
            return None;
        }
        let mut signed = Self {
            values: Vec::with_capacity(self.values.len()),
            types: Vec::with_capacity(self.types.len()),
            null_bitmap: self.null_bitmap.clone(),
            ends: Vec::with_capacity(self.ends.len()),
        };
        for (ty, bytes) in values() {
            if !misread(ty, bytes) {
                signed.values.extend_from_slice(bytes);
                signed.types.push(ty.clone());
            } else {
                let mut wide = [0; 8];
                wide[..bytes.len()].copy_from_slice(bytes);
                let value = u64::from_le_bytes(wide);
                match i64::try_from(value) {
                    Ok(value) => {
                        signed.values.extend_from_slice(&value.to_le_bytes());
                        signed.types.push(<i64 as Type<MySql>>::type_info());
                    }
                    Err(_) => {
                        signed.values.put_str_lenenc(&value.to_string());
                        signed
                            .types
                            .push(MySqlTypeInfo::binary(ColumnType::NewDecimal));
                    }
                }
            }
            signed.ends.push(signed.values.len());
        }
        Some(signed)
    }
}

impl Arguments for MySqlArguments {
    type Database = MySql;

    fn reserve(&mut self, len: usize, size: usize) {
        self.types.reserve(len);
        self.values.reserve(size);
    }

    fn add<'t, T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'t, Self::Database> + Type<Self::Database>,
    {
        self.add(value)
    }

    fn len(&self) -> usize {
        self.types.len()
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct NullBitMap {
    bytes: Vec<u8>,
    length: usize,
}

impl NullBitMap {
    fn push(&mut self, is_null: IsNull) {
        let byte_index = self.length / (u8::BITS as usize);
        let bit_offset = self.length % (u8::BITS as usize);

        if bit_offset == 0 {
            self.bytes.push(0);
        }

        self.bytes[byte_index] |= u8::from(is_null.is_null()) << bit_offset;
        self.length += 1;
    }
}

impl Deref for NullBitMap {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[cfg(test)]
#[path = "../tests/unit/arguments_tests.rs"]
mod test;
