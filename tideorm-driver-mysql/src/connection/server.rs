/// The server a connection reached, as its handshake named it. Each speaks
/// MySQL's protocol, but they differ in the SQL and session settings they
/// take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MySqlServer {
    /// MySQL, or a server naming no other kind.
    MySql,
    /// MariaDB (`5.5.5-10.11.6-MariaDB`).
    MariaDb,
    /// TiDB (`8.0.11-TiDB-v8.5.0`).
    TiDb,
    /// Vitess's `vtgate` (`8.0.40-Vitess`), which refuses `PIPES_AS_CONCAT`.
    Vitess,
}

impl MySqlServer {
    /// The server a handshake's version string names. A proxy configured to
    /// report a plain MySQL version reads as MySQL here; a Vitess that does
    /// is recognized when it refuses the session setup's `PIPES_AS_CONCAT`.
    pub fn from_version(version: &str) -> Self {
        let version = version.to_ascii_lowercase();
        if version.contains("mariadb") {
            Self::MariaDb
        } else if version.contains("-tidb-") {
            Self::TiDb
        } else if version.contains("-vitess") {
            Self::Vitess
        } else {
            Self::MySql
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/connection_server_tests.rs"]
mod tests;
