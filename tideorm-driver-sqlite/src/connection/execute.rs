use crate::connection::{ConnectionHandle, ConnectionState};
use crate::error::Error;
use crate::statement::{StatementHandle, VirtualStatement};
use crate::{SqliteArguments, SqliteQueryResult, SqliteRow};
use driver_core::arguments::Arguments;
use driver_core::sql_str::SqlSafeStr;
use driver_core::Either;

pub struct ExecuteIter<'a> {
    handle: &'a mut ConnectionHandle,
    shared: &'a super::worker::WorkerSharedState,
    statement: &'a mut VirtualStatement,
    args: Option<SqliteArguments>,

    /// since a `VirtualStatement` can encompass multiple actual statements,
    /// this keeps track of the number of arguments so far
    args_used: usize,

    /// The highest value a parameter has taken so far.
    highest_used: usize,

    goto_next: bool,
}

pub(crate) fn iter<'a>(
    conn: &'a mut ConnectionState,
    shared: &'a super::worker::WorkerSharedState,
    query: impl SqlSafeStr,
    args: Option<SqliteArguments>,
    persistent: bool,
) -> Result<ExecuteIter<'a>, Error> {
    let query = query.into_sql_str();
    // fetch the cached statement or allocate a new one
    let statement = conn.statements.get(query.as_str(), persistent)?;

    Ok(ExecuteIter {
        handle: &mut conn.handle,
        shared,
        statement,
        args,
        args_used: 0,
        highest_used: 0,
        goto_next: true,
    })
}

fn bind(
    statement: &mut StatementHandle,
    arguments: &Option<SqliteArguments>,
    offset: usize,
) -> Result<(usize, usize), Error> {
    match arguments {
        Some(arguments) => arguments.bind(statement, offset),
        None => Ok((0, 0)),
    }
}

/// The error for values no parameter took, once every statement is bound.
fn unused_values(arguments: &Option<SqliteArguments>, used: usize) -> Option<Error> {
    let given = arguments.as_ref().map_or(0, Arguments::len);
    (given > used).then(|| {
        Error::InvalidArgument(format!(
            "{given} values were given, but the statement's parameters take {used}"
        ))
    })
}

impl Iterator for ExecuteIter<'_> {
    type Item = Result<Either<SqliteQueryResult, SqliteRow>, Error>;

    /// The transaction state changes only between statements: one starts only
    /// while SQLite still holds the transaction this connection began, and the
    /// state is read again once it ends.
    fn next(&mut self) -> Option<Self::Item> {
        if self.goto_next {
            if let Err(error) = self.shared.ensure_transaction_active() {
                return Some(Err(error));
            }
        }
        let result = self.next_row();
        if !matches!(result, Some(Ok(Either::Right(_)))) {
            self.shared.update_transaction_state(self.handle);
        }
        result
    }
}

impl ExecuteIter<'_> {
    fn next_row(&mut self) -> Option<Result<Either<SqliteQueryResult, SqliteRow>, Error>> {
        if self.goto_next {
            let used = self.args_used.max(self.highest_used);
            let statement = match self.statement.prepare_next(self.handle) {
                Ok(Some(statement)) => statement,
                Ok(None) => return unused_values(&self.args, used).map(Err),
                Err(e) => return Some(Err(e)),
            };

            self.goto_next = false;

            // sanity check: ensure the VM is reset and the bindings are cleared
            if let Err(e) = statement.handle.reset() {
                return Some(Err(e.into()));
            }

            statement.handle.clear_bindings();

            match bind(statement.handle, &self.args, self.args_used) {
                Ok((anonymous, highest)) => {
                    self.args_used += anonymous;
                    self.highest_used = self.highest_used.max(highest);
                }
                Err(e) => return Some(Err(e)),
            }

            // Values left over are refused before the last statement runs.
            if self.statement.at_last() {
                let used = self.args_used.max(self.highest_used);
                if let Some(error) = unused_values(&self.args, used) {
                    return Some(Err(error));
                }
            }
        }
        let statement = self.statement.current()?;

        match statement.handle.step() {
            Ok(true) => {
                // shortcut: a re-prepare that renames a column but keeps their number
                // keeps the old names; compare the names too if that ever matters.
                if statement.handle.column_count() != statement.columns.len() {
                    self.statement.describe_current_again();
                }
                let statement = self.statement.current()?;
                Some(Ok(Either::Right(SqliteRow::current(
                    statement.handle,
                    statement.columns,
                    statement.column_names,
                ))))
            }
            Ok(false) => {
                let last_insert_rowid = self.handle.last_insert_rowid();

                let changes = statement.handle.changes();

                let done = SqliteQueryResult {
                    changes,
                    last_insert_rowid,
                };

                self.goto_next = true;

                Some(Ok(Either::Left(done)))
            }
            Err(e) => Some(Err(e.into())),
        }
    }
}

impl Drop for ExecuteIter<'_> {
    fn drop(&mut self) {
        self.statement.reset().ok();
        self.shared.update_transaction_state(self.handle);
    }
}
