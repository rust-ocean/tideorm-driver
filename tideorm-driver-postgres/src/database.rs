use crate::arguments::PgArgumentBuffer;
use crate::value::{PgValue, PgValueRef};
use crate::{
    PgArguments, PgColumn, PgConnection, PgQueryResult, PgRow, PgStatement, PgTransactionManager,
    PgTypeInfo,
};

pub(crate) use driver_core::database::{Database, HasStatementCache};

/// PostgreSQL database driver.
#[derive(Debug)]
pub struct Postgres;

impl Database for Postgres {
    type Connection = PgConnection;

    type TransactionManager = PgTransactionManager;

    type Row = PgRow;

    type QueryResult = PgQueryResult;

    type Column = PgColumn;

    type TypeInfo = PgTypeInfo;

    type Value = PgValue;
    type ValueRef<'r> = PgValueRef<'r>;

    type Arguments = PgArguments;
    type ArgumentBuffer = PgArgumentBuffer;

    type Statement = PgStatement;

    const NAME: &'static str = "PostgreSQL";

    /// The prepared-statement protocol carries a 16-bit parameter count.
    const MAX_BIND_PARAMETERS: usize = u16::MAX as usize;

    const URL_SCHEMES: &'static [&'static str] = &["postgres", "postgresql"];
}

impl HasStatementCache for Postgres {}
