//! Writing PostgreSQL frames: length prefixes and the names of prepared statements and portals.

use std::num::{NonZeroU32, Saturating};

pub(crate) use driver_core::io::*;

/// Writes the framing parts of PostgreSQL messages.
pub trait PgBufMutExt {
    /// Appends a message body behind its length, an `Int32` that counts itself and the body.
    ///
    /// When `write_body` fails, or the message passes `i32::MAX` bytes, the buffer is put back
    /// as it was and the error returned.
    fn put_length_prefixed<F>(&mut self, write_body: F) -> Result<(), crate::Error>
    where
        F: FnOnce(&mut Vec<u8>) -> Result<(), crate::Error>;

    /// Appends a prepared statement's name and a NUL (only the NUL for the unnamed one).
    fn put_statement_name(&mut self, id: StatementId);

    /// Appends a portal's name and a NUL (only the NUL for the unnamed one).
    fn put_portal_name(&mut self, id: PortalId);
}

impl PgBufMutExt for Vec<u8> {
    fn put_length_prefixed<F>(&mut self, write_body: F) -> Result<(), crate::Error>
    where
        F: FnOnce(&mut Vec<u8>) -> Result<(), crate::Error>,
    {
        let start = self.len();
        self.extend_from_slice(&[0; 4]);

        let framed = write_body(self).and_then(|()| {
            let length = self.len() - start;
            let length = i32::try_from(length)
                .map_err(|_| err_protocol!("message size out of range for protocol: {length}"))?;
            self[start..start + 4].copy_from_slice(&length.to_be_bytes());
            Ok(())
        });

        if framed.is_err() {
            self.truncate(start);
        }
        framed
    }

    #[inline]
    fn put_statement_name(&mut self, id: StatementId) {
        id.put_name_with_nul(self);
    }

    #[inline]
    fn put_portal_name(&mut self, id: PortalId) {
        id.put_name_with_nul(self);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct StatementId(IdInner);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct PortalId(IdInner);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct IdInner(Option<NonZeroU32>);

impl StatementId {
    pub const UNNAMED: Self = Self(IdInner::UNNAMED);

    pub const NAMED_START: Self = Self(IdInner::NAMED_START);

    #[cfg(test)]
    pub const TEST_VAL: Self = Self(IdInner::TEST_VAL);

    const NAME_PREFIX: &'static str = "tide_s_";

    pub fn next(&self) -> Self {
        Self(self.0.next())
    }

    pub fn name_len(&self) -> Saturating<usize> {
        self.0.name_len(Self::NAME_PREFIX)
    }

    pub fn put_name_with_nul(&self, buf: &mut Vec<u8>) {
        self.0.put_name_with_nul(Self::NAME_PREFIX, buf)
    }
}

impl PortalId {
    // None selects the unnamed portal
    pub const UNNAMED: Self = PortalId(IdInner::UNNAMED);

    const NAME_PREFIX: &'static str = "tide_p_";

    /// Calculate the number of bytes that will be written by [`Self::put_name_with_nul()`].
    pub fn name_len(&self) -> Saturating<usize> {
        self.0.name_len(Self::NAME_PREFIX)
    }

    pub fn put_name_with_nul(&self, buf: &mut Vec<u8>) {
        self.0.put_name_with_nul(Self::NAME_PREFIX, buf)
    }
}

impl IdInner {
    const UNNAMED: Self = Self(None);

    const NAMED_START: Self = Self(Some(NonZeroU32::MIN));

    #[cfg(test)]
    pub const TEST_VAL: Self = Self(NonZeroU32::new(1234567890));

    #[inline(always)]
    fn next(&self) -> Self {
        Self(
            self.0
                .map(|id| id.checked_add(1).unwrap_or(NonZeroU32::MIN)),
        )
    }

    #[inline(always)]
    fn name_len(&self, name_prefix: &str) -> Saturating<usize> {
        let mut len = Saturating(0);

        if let Some(id) = self.0 {
            len += name_prefix.len();
            // estimate the length of the ID in decimal
            // `.ilog10()` can't panic since the value is never zero
            len += id.get().ilog10() as usize;
            // add one to compensate for `ilog10()` rounding down.
            len += 1;
        }

        // count the NUL terminator
        len += 1;

        len
    }

    #[inline(always)]
    fn put_name_with_nul(&self, name_prefix: &str, buf: &mut Vec<u8>) {
        if let Some(id) = self.0 {
            buf.extend_from_slice(name_prefix.as_bytes());
            buf.extend_from_slice(itoa::Buffer::new().format(id.get()).as_bytes());
        }

        buf.push(0);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/io_tests.rs"]
mod tests;
