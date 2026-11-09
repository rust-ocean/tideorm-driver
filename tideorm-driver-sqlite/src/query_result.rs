use std::iter::{Extend, IntoIterator};

#[derive(Debug, Default)]
pub struct SqliteQueryResult {
    pub(super) changes: u64,
    pub(super) last_insert_rowid: i64,
}

impl SqliteQueryResult {
    pub fn rows_affected(&self) -> u64 {
        self.changes
    }

    pub fn last_insert_rowid(&self) -> i64 {
        self.last_insert_rowid
    }
}

impl driver_core::database::QueryResult for SqliteQueryResult {
    fn rows_affected(&self) -> u64 {
        self.changes
    }

    fn last_insert_id(&self) -> Option<i128> {
        Some(i128::from(self.last_insert_rowid))
    }
}

impl Extend<SqliteQueryResult> for SqliteQueryResult {
    fn extend<T: IntoIterator<Item = SqliteQueryResult>>(&mut self, iter: T) {
        for elem in iter {
            self.changes += elem.changes;
            self.last_insert_rowid = elem.last_insert_rowid;
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/query_result_tests.rs"]
mod tests;
