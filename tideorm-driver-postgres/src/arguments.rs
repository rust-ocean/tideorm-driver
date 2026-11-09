use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::encode::{Encode, IsNull};
use crate::error::Error;
use crate::types::Type;
use crate::{PgConnection, PgTypeInfo, Postgres};

pub(crate) use driver_core::arguments::Arguments;
use driver_core::error::BoxDynError;

/// The encoded values of a statement's parameters, as `Bind` sends them: each one a big-endian
/// `i32` byte count (`-1` for NULL) and then its bytes.
///
/// Some encodings depend on facts known only once the statement is prepared. Those write a
/// placeholder now, which `PgArguments::apply_patches` completes:
/// - a *patch* rewrites bytes once the parameter's declared type is known (a `Json<T>` sent to
///   a `JSON` parameter rather than a `JSONB` one);
/// - a *hole* is a type OID left as zero for a type known only by name (in an array or record
///   of a custom type), filled once the connection has looked the name up.
#[derive(Default, Debug, Clone)]
pub struct PgArgumentBuffer {
    buffer: Vec<u8>,

    /// How many arguments have been added.
    count: usize,

    patches: Vec<Patch>,

    /// Where each hole is in `buffer`; `hole_types` holds its type, in a vector of its own
    /// because `PgConnection::resolve_types` takes a slice of them.
    hole_offsets: Vec<usize>,
    hole_types: Vec<PgTypeInfo>,
}

type PatchFn = Arc<dyn Fn(&mut [u8], &PgTypeInfo) + Send + Sync>;

/// Rewrites the bytes from `buf_offset` once argument `arg_index`'s declared type is known.
#[derive(Clone)]
struct Patch {
    buf_offset: usize,
    arg_index: usize,
    callback: PatchFn,
}

impl fmt::Debug for Patch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Patch")
            .field("buf_offset", &self.buf_offset)
            .field("arg_index", &self.arg_index)
            .finish_non_exhaustive()
    }
}

/// The parameters of one statement, collected through [`Arguments`].
#[derive(Default, Debug, Clone)]
pub struct PgArguments {
    /// Each parameter's type, in order; `Parse` declares them to the server.
    pub(crate) types: Vec<PgTypeInfo>,

    pub(crate) buffer: PgArgumentBuffer,
}

impl PgArguments {
    pub(crate) fn add<'q, T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'q, Postgres> + Type<Postgres>,
    {
        let type_info = value.produces().unwrap_or_else(T::type_info);
        self.buffer.push_argument(value)?;
        self.types.push(type_info);
        Ok(())
    }

    /// Completes what the encoders left open: runs every patch against its parameter's
    /// declared type, then writes the OID of every type left as a hole (the connection asks the
    /// server only for names it has not resolved before).
    pub(crate) async fn apply_patches(
        &mut self,
        conn: &mut PgConnection,
        parameters: &[PgTypeInfo],
    ) -> Result<(), Error> {
        let PgArgumentBuffer {
            buffer,
            patches,
            hole_offsets,
            hole_types,
            ..
        } = &mut self.buffer;

        for patch in patches.iter() {
            (patch.callback)(
                &mut buffer[patch.buf_offset..],
                &parameters[patch.arg_index],
            );
        }

        let oids = conn.resolve_types(hole_types).await?;
        for (&offset, oid) in hole_offsets.iter().zip(oids) {
            buffer[offset..offset + 4].copy_from_slice(&oid.0.to_be_bytes());
        }

        Ok(())
    }
}

impl Arguments for PgArguments {
    type Database = Postgres;

    fn reserve(&mut self, additional: usize, size: usize) {
        self.types.reserve(additional);
        self.buffer.reserve(size);
    }

    fn add<'t, T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'t, Self::Database> + Type<Self::Database>,
    {
        self.add(value)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.buffer.count
    }
}

impl PgArgumentBuffer {
    /// Adds `value` as the next argument. If encoding fails, the buffer is left as it was, with
    /// no half-written value or stray patch behind.
    fn push_argument<'q, T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'q, Postgres>,
    {
        let (bytes, patches, holes) = (
            self.buffer.len(),
            self.patches.len(),
            self.hole_offsets.len(),
        );

        let result = self.encode(value);
        if result.is_ok() {
            self.count += 1;
        } else {
            self.buffer.truncate(bytes);
            self.patches.truncate(patches);
            self.hole_offsets.truncate(holes);
            self.hole_types.truncate(holes);
        }
        result
    }

    /// Writes `value` as one length-prefixed field: its byte count as a big-endian `i32`, then
    /// its bytes; a NULL is the count `-1` alone. Arrays and records write their elements
    /// through this too.
    pub(crate) fn encode<'q, T>(&mut self, value: T) -> Result<(), BoxDynError>
    where
        T: Encode<'q, Postgres>,
    {
        // refuse early a value that already says it is too big
        int4_len(value.size_hint())?;

        let start = self.len();
        self.extend_from_slice(&[0; 4]);

        let len = match value.encode(self)? {
            IsNull::No => int4_len(self.len() - start - 4)?,
            IsNull::Yes => {
                debug_assert_eq!(self.len(), start + 4, "a NULL must write no bytes");
                -1
            }
        };
        self[start..start + 4].copy_from_slice(&len.to_be_bytes());

        Ok(())
    }

    /// Registers `callback` to rewrite the bytes from here on once the current argument's
    /// declared type is known.
    pub(crate) fn patch_with<F>(&mut self, callback: F)
    where
        F: Fn(&mut [u8], &PgTypeInfo) + 'static + Send + Sync,
    {
        self.patches.push(Patch {
            buf_offset: self.len(),
            arg_index: self.count,
            callback: Arc::new(callback),
        });
    }

    /// Writes a zero OID to be replaced by `type_info`'s once the connection has resolved it.
    pub(crate) fn push_hole(&mut self, type_info: PgTypeInfo) {
        self.hole_offsets.push(self.len());
        self.hole_types.push(type_info);
        self.extend_from_slice(&0_u32.to_be_bytes());
    }
}

impl Deref for PgArgumentBuffer {
    type Target = Vec<u8>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for PgArgumentBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

/// `size` as the protocol's `i32` length, or an error if it does not fit.
fn int4_len(size: usize) -> Result<i32, String> {
    i32::try_from(size).map_err(|_| {
        format!(
            "value size would overflow in the binary protocol encoding: {size} > {}",
            i32::MAX
        )
    })
}
