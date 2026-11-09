//! Prepared statements.

use crate::arguments::IntoArguments;
use crate::column::ColumnIndex;
use crate::database::Database;
use crate::error::Error;
use crate::query::Query;
use crate::sql_str::SqlStr;
use either::Either;

/// A statement the server has prepared, with what it reported about the statement's
/// parameters and result columns.
///
/// Each connection prepares and caches the statements it runs by itself; preparing one
/// explicitly is a way to read that description before running it. A statement is not
/// tied to the connection that prepared it: another connection prepares (and caches) it
/// again the first time it runs it.
pub trait Statement: Send + Sync + Clone {
    type Database: Database;

    /// The SQL the statement was prepared from.
    fn sql(&self) -> &SqlStr;

    /// The statement's parameters, as far as the backend describes them: their types
    /// (PostgreSQL) or only how many there are (SQLite).
    fn parameters(&self) -> Option<Either<&[<Self::Database as Database>::TypeInfo], usize>>;

    /// The columns a run of the statement returns.
    fn columns(&self) -> &[<Self::Database as Database>::Column];

    /// The column `index` names: a `usize` position or a `&str` name.
    ///
    /// # Panics
    ///
    /// When there is no such column; [`try_column`](Self::try_column) returns an error
    /// instead.
    fn column<I>(&self, index: I) -> &<Self::Database as Database>::Column
    where
        I: ColumnIndex<Self>,
    {
        self.try_column(index).unwrap()
    }

    /// The column `index` names, or the [`ColumnIndex`] error for a missing one.
    fn try_column<I>(&self, index: I) -> Result<&<Self::Database as Database>::Column, Error>
    where
        I: ColumnIndex<Self>,
    {
        Ok(&self.columns()[index.index(self)?])
    }

    /// A query running this statement, with arguments to be bound to it.
    fn query(&self) -> Query<'_, Self::Database, <Self::Database as Database>::Arguments>;

    /// A query running this statement with `arguments`.
    fn query_with<A>(&self, arguments: A) -> Query<'_, Self::Database, A>
    where
        A: IntoArguments<Self::Database>;
}

/// Writes [`Statement::query`] and [`Statement::query_with`] inside a backend's
/// `impl Statement`, given its `Arguments` type.
#[macro_export]
macro_rules! impl_statement_query {
    ($A:ty) => {
        #[inline]
        fn query(&self) -> $crate::query::Query<'_, Self::Database, $A> {
            $crate::query::query_statement(self)
        }

        #[inline]
        fn query_with<A>(&self, arguments: A) -> $crate::query::Query<'_, Self::Database, A>
        where
            A: $crate::arguments::IntoArguments<Self::Database>,
        {
            $crate::query::query_statement_with(self, arguments)
        }
    };
}
