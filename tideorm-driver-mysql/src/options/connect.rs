use crate::connection::ConnectOptions;
use crate::error::Error;
use crate::executor::Executor;
use crate::{MySqlConnectOptions, MySqlConnection};
use driver_core::sql_str::AssertSqlSafe;
use driver_core::Url;

impl ConnectOptions for MySqlConnectOptions {
    type Connection = MySqlConnection;

    fn from_url(url: &Url) -> Result<Self, Error> {
        Self::parse_from_url(url)
    }

    async fn connect(&self) -> Result<Self::Connection, Error>
    where
        Self::Connection: Sized,
    {
        let mut conn = MySqlConnection::establish(self).await?;

        if let Some(sql) = self.session_setup() {
            conn.execute(AssertSqlSafe(sql)).await?;
        }

        Ok(conn)
    }
}

impl MySqlConnectOptions {
    /// The one `SET` a new connection runs before it is used, or `None` when
    /// these options leave the session as the server opened it.
    ///
    /// - `PIPES_AS_CONCAT` makes `||` concatenate strings, not `OR` them.
    /// - `NO_ENGINE_SUBSTITUTION` makes a `CREATE TABLE` naming a missing
    ///   storage engine fail, not quietly use the default one.
    /// - `time_zone` is what `TIMESTAMP` values are read and written in
    ///   (UTC by default).
    /// - `SET NAMES` makes the server send and expect text in the chosen
    ///   character set, whatever each column stores; with no collation the
    ///   server picks that character set's default.
    ///
    /// <https://dev.mysql.com/doc/refman/8.4/en/sql-mode.html>
    /// <https://dev.mysql.com/doc/refman/8.4/en/set-names.html>
    fn session_setup(&self) -> Option<String> {
        let sql_modes: Vec<&str> = [
            (self.pipes_as_concat, "PIPES_AS_CONCAT"),
            (self.no_engine_substitution, "NO_ENGINE_SUBSTITUTION"),
        ]
        .into_iter()
        .filter_map(|(on, mode)| on.then_some(mode))
        .collect();

        let mut assignments = Vec::new();
        if !sql_modes.is_empty() {
            // added to the server's own modes, not in place of them
            assignments.push(format!(
                "sql_mode=(SELECT CONCAT(@@sql_mode, ',{}'))",
                sql_modes.join(",")
            ));
        }
        if let Some(timezone) = &self.timezone {
            assignments.push(format!("time_zone='{timezone}'"));
        }
        if self.set_names {
            assignments.push(match &self.collation {
                Some(collation) => format!("NAMES {} COLLATE {collation}", self.charset),
                None => format!("NAMES {}", self.charset),
            });
        }

        (!assignments.is_empty()).then(|| format!("SET {};", assignments.join(",")))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/options_connect_tests.rs"]
mod tests;
