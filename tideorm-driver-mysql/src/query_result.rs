use std::iter::{Extend, IntoIterator};

#[derive(Debug, Default)]
pub struct MySqlQueryResult {
    pub(super) rows_affected: u64,
    pub(super) last_insert_id: u64,
}

impl MySqlQueryResult {
    pub fn last_insert_id(&self) -> u64 {
        self.last_insert_id
    }

    pub fn rows_affected(&self) -> u64 {
        self.rows_affected
    }
}

impl driver_core::database::QueryResult for MySqlQueryResult {
    fn rows_affected(&self) -> u64 {
        self.rows_affected
    }

    fn last_insert_id(&self) -> Option<i128> {
        Some(i128::from(self.last_insert_id))
    }
}

impl Extend<MySqlQueryResult> for MySqlQueryResult {
    fn extend<T: IntoIterator<Item = MySqlQueryResult>>(&mut self, iter: T) {
        for elem in iter {
            self.rows_affected += elem.rows_affected;
            self.last_insert_id = elem.last_insert_id;
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/query_result_tests.rs"]
mod tests;
