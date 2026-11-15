/// The server a connection reached, as the parameters it reported when the
/// connection opened named it. Each speaks PostgreSQL's protocol, but they
/// differ in the SQL and settings they take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PgServer {
    /// PostgreSQL, or a server naming no other kind.
    Postgres,
    /// CockroachDB, which reports a `crdb_version` parameter.
    CockroachDb,
    /// YugabyteDB, whose `server_version` reads like `15.12-YB-2025.2.8.1-b0`.
    YugabyteDb,
}

impl PgServer {
    /// The server after it reported parameter `name` as `value`.
    pub(crate) fn reported(self, name: &str, value: &str) -> Self {
        match name {
            "crdb_version" => Self::CockroachDb,
            "server_version" if self == Self::Postgres && value.contains("-YB-") => {
                Self::YugabyteDb
            }
            _ => self,
        }
    }

    /// The statement a new connection runs before it is used, if any.
    /// CockroachDB's makes SQL written for PostgreSQL mean there what it
    /// means on PostgreSQL: a `SERIAL` column takes its
    /// values from a sequence (not `unique_rowid()`'s 64-bit ones), DDL in a
    /// transaction stays in it instead of committing what ran before it, and a
    /// new table takes schema changes inside a transaction. A variable the
    /// server's version lacks is left out, not an error.
    ///
    /// Two settings are left alone. `default_int_size` stays 8, so a plain `INT`
    /// or `SERIAL` is 64-bit there (`INT4`/`SERIAL4` are 32-bit): 4 would cap
    /// every sequence, `BIGSERIAL`'s too, at 2^31 - 1. `NULL`s keep sorting
    /// first in ascending order: `null_ordered_last` makes CockroachDB refuse a
    /// `SELECT DISTINCT` ordered by a nullable column.
    ///
    /// <https://www.cockroachlabs.com/docs/stable/session-variables>
    pub(crate) fn session_setup(self) -> Option<&'static str> {
        (self == Self::CockroachDb).then_some(
            "SELECT set_config(s.name, s.value, false) \
                 FROM (VALUES ('serial_normalization', 'sql_sequence'), \
                 ('autocommit_before_ddl', 'off'), \
                 ('create_table_with_schema_locked', 'off')) AS s (name, value) \
                 WHERE s.name IN (SELECT name FROM pg_catalog.pg_settings)",
        )
    }
}

#[cfg(test)]
#[path = "../../tests/unit/connection_server_tests.rs"]
mod tests;
