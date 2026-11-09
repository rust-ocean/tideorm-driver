use std::borrow::Cow;
use std::str::from_utf8;

use bytes::Bytes;
pub(crate) use driver_core::value::*;

use crate::error::{BoxDynError, UnexpectedNullError};
use crate::protocol::text::ColumnType;
use crate::{MySql, MySqlTypeInfo};

/// How a value's bytes are laid out, which depends on how its statement ran.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum MySqlValueFormat {
    /// Every value as text: the result of a statement run without arguments.
    Text,
    /// Each type's own binary layout: the result of a prepared statement.
    Binary,
}

/// Implementation of [`Value`] for MySQL: a value that owns its bytes.
#[derive(Clone)]
pub struct MySqlValue {
    value: Option<Bytes>,
    type_info: MySqlTypeInfo,
    format: MySqlValueFormat,
}

/// Implementation of [`ValueRef`] for MySQL: a value borrowed from its row.
#[derive(Clone)]
pub struct MySqlValueRef<'r> {
    /// The value's bytes, or `None` for SQL `NULL`.
    pub(crate) value: Option<&'r [u8]>,
    /// The row `value` points into, which lets `to_owned` share it rather
    /// than copy it.
    pub(crate) row: Option<&'r Bytes>,
    pub(crate) type_info: MySqlTypeInfo,
    pub(crate) format: MySqlValueFormat,
}

impl<'r> MySqlValueRef<'r> {
    pub(crate) fn format(&self) -> MySqlValueFormat {
        self.format
    }

    pub(crate) fn as_bytes(&self) -> Result<&'r [u8], BoxDynError> {
        self.value.ok_or_else(|| UnexpectedNullError.into())
    }

    pub(crate) fn as_str(&self) -> Result<&'r str, BoxDynError> {
        Ok(from_utf8(self.as_bytes()?)?)
    }
}

impl Value for MySqlValue {
    type Database = MySql;

    fn as_ref(&self) -> MySqlValueRef<'_> {
        MySqlValueRef {
            value: self.value.as_deref(),
            row: None,
            type_info: self.type_info.clone(),
            format: self.format,
        }
    }

    fn type_info(&self) -> Cow<'_, MySqlTypeInfo> {
        Cow::Borrowed(&self.type_info)
    }

    fn is_null(&self) -> bool {
        is_null(self.value.as_deref(), &self.type_info)
    }
}

impl<'r> ValueRef<'r> for MySqlValueRef<'r> {
    type Database = MySql;

    fn to_owned(&self) -> MySqlValue {
        let value = self.value.map(|value| match self.row {
            Some(row) => row.slice_ref(value),
            None => Bytes::copy_from_slice(value),
        });

        MySqlValue {
            value,
            format: self.format,
            type_info: self.type_info.clone(),
        }
    }

    fn type_info(&self) -> Cow<'_, MySqlTypeInfo> {
        Cow::Borrowed(&self.type_info)
    }

    #[inline]
    fn is_null(&self) -> bool {
        is_null(self.value, &self.type_info)
    }
}

/// SQL `NULL`, or a binary zero date (`0000-00-00`, sent as length 0), which
/// no Rust date can hold.
fn is_null(value: Option<&[u8]>, ty: &MySqlTypeInfo) -> bool {
    match value {
        None => true,
        Some(bytes) => {
            bytes.first() == Some(&0)
                && matches!(
                    ty.r#type,
                    ColumnType::Date | ColumnType::Timestamp | ColumnType::Datetime
                )
        }
    }
}
