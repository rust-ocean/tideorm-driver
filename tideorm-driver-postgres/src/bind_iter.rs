use crate::{PgArgumentBuffer, PgHasArrayType, PgTypeInfo, Postgres};
use core::cell::Cell;
use driver_core::{
    encode::{Encode, IsNull},
    error::BoxDynError,
    types::Type,
};

/// An iterator bound as an array, made by [`PgBindIterExt::bind_iter`].
///
/// Public only because the trait names it; it is not exported. Binding encodes through a shared
/// reference, so the iterator waits in a `Cell` to be taken out and run then, once.
pub struct PgBindIter<I>(Cell<Option<I>>);

/// Binds an iterator as a one-dimensional PostgreSQL array, encoding each item as it comes.
///
/// Nothing is collected first: the items go straight into the statement's parameters. That
/// suits `unnest`, which inserts many rows with one statement and one array per column. Each
/// column can come from a single pass over the rows, borrowing what it does not copy, since a
/// reference binds as what it points to:
///
/// ```rust,no_run
/// # async fn insert_readings() -> Result<(), driver::error::BoxDynError> {
/// # use driver::Connection;
/// # let mut conn = <driver::Postgres as driver::Database>::Connection::connect("postgres://").await?;
/// use driver::postgres::PgBindIterExt;
///
/// struct Reading {
///     sensor: i32,
///     celsius: f64,
///     note: String,
/// }
///
/// let readings: Vec<Reading> = Vec::new();
///
/// driver::query(
///     "INSERT INTO readings (sensor, celsius, note) SELECT * FROM unnest($1, $2, $3)",
/// )
/// .bind(readings.iter().map(|r| r.sensor).bind_iter())
/// .bind(readings.iter().map(|r| r.celsius).bind_iter())
/// .bind(readings.iter().map(|r| &r.note).bind_iter())
/// .execute(&mut conn)
/// .await?;
/// # Ok(())
/// # }
/// ```
pub trait PgBindIterExt: Iterator + Sized {
    fn bind_iter(self) -> PgBindIter<Self>;
}

impl<I: Iterator + Sized> PgBindIterExt for I {
    fn bind_iter(self) -> PgBindIter<I> {
        PgBindIter(Cell::new(Some(self)))
    }
}

impl<I> Type<Postgres> for PgBindIter<I>
where
    I: Iterator,
    <I as Iterator>::Item: Type<Postgres> + PgHasArrayType,
{
    fn type_info() -> PgTypeInfo {
        <I as Iterator>::Item::array_type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <I as Iterator>::Item::array_compatible(ty)
    }
}

impl<'q, I> PgBindIter<I>
where
    I: Iterator,
    <I as Iterator>::Item: Type<Postgres> + Encode<'q, Postgres>,
{
    /// Writes the items as a binary array: the dimension count (1), a flags word, the element
    /// type's OID, this dimension's length and lower bound (1), then each element as a
    /// length-prefixed field.
    fn encode_items(iter: I, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        let at_least = iter.size_hint().0;
        let mut items = iter.peekable();

        // A value can name a more precise type than its Rust type does (`Encode::produces`);
        // the first one decides the element type.
        let element = items
            .peek()
            .and_then(Encode::produces)
            .unwrap_or_else(<I as Iterator>::Item::type_info);

        buf.extend(1_i32.to_be_bytes());
        buf.extend(0_i32.to_be_bytes());
        match element.oid() {
            Some(oid) => buf.extend(oid.0.to_be_bytes()),
            None => buf.push_hole(element),
        }
        let len_at = buf.len();
        buf.extend(0_i32.to_be_bytes()); // the length, written once counted
        buf.extend(1_i32.to_be_bytes());

        let mut len = 0_i32;
        for item in items {
            if len == i32::MAX {
                const TOO_MANY: usize = i32::MAX as usize + 1;
                let size = at_least.max(TOO_MANY);
                return Err(format!("encoded iterator is too large for Postgres: {size}").into());
            }
            buf.encode(item)?;
            len += 1;
        }
        buf[len_at..len_at + 4].copy_from_slice(&len.to_be_bytes());

        Ok(IsNull::No)
    }
}

impl<'q, I> Encode<'q, Postgres> for PgBindIter<I>
where
    I: Iterator,
    <I as Iterator>::Item: Type<Postgres> + Encode<'q, Postgres>,
{
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        Self::encode_items(self.0.take().expect("PgBindIter is only used once"), buf)
    }

    fn encode(self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError>
    where
        Self: Sized,
    {
        Self::encode_items(
            self.0.into_inner().expect("PgBindIter is only used once"),
            buf,
        )
    }
}
