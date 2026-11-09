use driver_core::bytes::Buf;

use crate::decode::Decode;
use crate::encode::Encode;
use crate::error::{mismatched_types, BoxDynError};
use crate::type_info::TypeInfo;
use crate::type_info::{PgType, PgTypeKind};
use crate::types::Oid;
use crate::types::Type;
use crate::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};

#[doc(hidden)]
pub struct PgRecordEncoder<'a> {
    buf: &'a mut PgArgumentBuffer,
    off: usize,
    num: u32,
}

impl<'a> PgRecordEncoder<'a> {
    #[doc(hidden)]
    pub fn new(buf: &'a mut PgArgumentBuffer) -> Self {
        let off = buf.len();

        // reserve space for a field count
        buf.extend(&(0_u32).to_be_bytes());

        Self { buf, off, num: 0 }
    }

    #[doc(hidden)]
    pub fn finish(&mut self) {
        // fill in the record length
        self.buf[self.off..(self.off + 4)].copy_from_slice(&self.num.to_be_bytes());
    }

    #[doc(hidden)]
    pub fn encode<'q, T>(&mut self, value: T) -> Result<&mut Self, BoxDynError>
    where
        'a: 'q,
        T: Encode<'q, Postgres> + Type<Postgres>,
    {
        let ty = value.produces().unwrap_or_else(T::type_info);

        if let Some(oid) = ty.oid() {
            self.buf.extend(oid.0.to_be_bytes())
        } else {
            self.buf.push_hole(ty);
        }

        self.buf.encode(value)?;
        self.num += 1;

        Ok(self)
    }
}

#[doc(hidden)]
pub struct PgRecordDecoder<'r> {
    buf: &'r [u8],
    typ: PgTypeInfo,
    fmt: PgValueFormat,
    ind: usize,
    /// In text, whether a field is left: the last one ended with a comma.
    more: bool,
}

impl<'r> PgRecordDecoder<'r> {
    #[doc(hidden)]
    pub fn new(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let fmt = value.format();
        let mut buf = value.as_bytes()?;
        let typ = value.type_info;

        match fmt {
            PgValueFormat::Binary => {
                let _len = buf.try_get_u32()?;
            }

            PgValueFormat::Text => {
                // remove the enclosing `(` .. `)`
                buf = buf
                    .strip_prefix(b"(")
                    .and_then(|inner| inner.strip_suffix(b")"))
                    .ok_or("expected a record in parentheses")?;
            }
        }

        Ok(Self {
            buf,
            fmt,
            typ,
            ind: 0,
            more: true,
        })
    }

    #[doc(hidden)]
    pub fn try_decode<T>(&mut self) -> Result<T, BoxDynError>
    where
        T: for<'a> Decode<'a, Postgres> + Type<Postgres>,
    {
        let missing = match self.fmt {
            PgValueFormat::Binary => self.buf.is_empty(),
            PgValueFormat::Text => !self.more,
        };
        if missing {
            return Err(format!("no field `{0}` found on record", self.ind).into());
        }

        match self.fmt {
            PgValueFormat::Binary => {
                let element_type_oid = Oid(self.buf.try_get_u32()?);
                let element_type_opt = self.find_type_info(&self.typ, element_type_oid)?;

                if let Some(ty) = &element_type_opt {
                    if !ty.is_null() && !T::compatible(ty) {
                        return Err(mismatched_types::<Postgres, T>(ty));
                    }
                }

                let element_type =
                    element_type_opt
                        .ok_or_else(|| BoxDynError::from(format!("custom types in records are not fully supported yet: failed to retrieve type info for field {} with type oid {}", self.ind, element_type_oid.0)))?;

                self.ind += 1;

                T::decode(PgValueRef::get(&mut self.buf, self.fmt, element_type)?)
            }

            PgValueFormat::Text => {
                // Read bytes, not chars: the delimiters are ASCII, which never
                // occurs inside a multi-byte UTF-8 character.
                let mut element = Vec::new();
                let mut quoted = false;
                let mut in_quotes = false;
                let mut in_escape = false;
                let mut prev = 0_u8;
                self.more = false;
                self.ind += 1;

                while let Some((&byte, rest)) = self.buf.split_first() {
                    self.buf = rest;
                    match byte {
                        _ if in_escape => {
                            element.push(byte);
                            in_escape = false;
                        }

                        b'"' if in_quotes => {
                            in_quotes = false;
                        }

                        b'"' => {
                            in_quotes = true;
                            quoted = true;

                            if prev == b'"' {
                                element.push(b'"')
                            }
                        }

                        b'\\' => {
                            in_escape = true;
                        }

                        b',' if !in_quotes => {
                            self.more = true;
                            break;
                        }

                        _ => {
                            element.push(byte);
                        }
                    }
                    prev = byte;
                }

                let buf = if element.is_empty() && !quoted {
                    // completely empty input means NULL
                    None
                } else {
                    Some(element.as_slice())
                };

                // NOTE: we do not call [`accepts`] or give a chance to from a user as
                //       TEXT sequences are not strongly typed

                T::decode(PgValueRef {
                    // NOTE: We pass `0` as the type ID because we don't have a reasonable value
                    //       we could use.
                    type_info: PgTypeInfo::with_oid(Oid(0)),
                    format: self.fmt,
                    value: buf,
                    row: None,
                })
            }
        }
    }

    fn find_type_info(
        &self,
        typ: &PgTypeInfo,
        oid: Oid,
    ) -> Result<Option<PgTypeInfo>, BoxDynError> {
        match typ.kind() {
            PgTypeKind::Simple if typ.0 == PgType::Record => Ok(PgTypeInfo::try_from_oid(oid)),
            PgTypeKind::Composite(fields) => {
                let ty = fields
                    .get(self.ind)
                    .ok_or_else(|| {
                        format!(
                            "the record has more fields than its type's {}",
                            fields.len()
                        )
                    })?
                    .1
                    .clone();
                if ty.0.oid() != oid {
                    return Err("unexpected mismatch of composite type information".into());
                }

                Ok(Some(ty))
            }
            PgTypeKind::Domain(domain) => self.find_type_info(domain, oid),
            _ => Err("unexpected custom type being decoded as a composite type".into()),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/types_record_tests.rs"]
mod tests;
