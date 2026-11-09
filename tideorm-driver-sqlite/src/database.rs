pub(crate) use driver_core::database::{Database, HasStatementCache};

use crate::arguments::SqliteArgumentsBuffer;
use crate::{
    SqliteArguments, SqliteColumn, SqliteConnection, SqliteQueryResult, SqliteRow, SqliteStatement,
    SqliteTransactionManager, SqliteTypeInfo, SqliteValue, SqliteValueRef,
};

/// Sqlite database driver.
#[derive(Debug)]
pub struct Sqlite;

impl Database for Sqlite {
    type Connection = SqliteConnection;

    type TransactionManager = SqliteTransactionManager;

    type Row = SqliteRow;

    type QueryResult = SqliteQueryResult;

    type Column = SqliteColumn;

    type TypeInfo = SqliteTypeInfo;

    type Value = SqliteValue;
    type ValueRef<'r> = SqliteValueRef<'r>;

    type Arguments = SqliteArguments;
    type ArgumentBuffer = SqliteArgumentsBuffer;

    type Statement = SqliteStatement;

    const NAME: &'static str = "SQLite";

    /// The variable-number limit of the bundled SQLite build.
    const MAX_BIND_PARAMETERS: usize = 32_766;

    const URL_SCHEMES: &'static [&'static str] = &["sqlite"];
}

impl HasStatementCache for Sqlite {}
