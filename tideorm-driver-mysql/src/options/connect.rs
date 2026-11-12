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
        let setup = self.session_setup()?;
        let mut conn = MySqlConnection::establish(self).await?;
        conn.execute(AssertSqlSafe(setup)).await?;
        Ok(conn)
    }
}

impl MySqlConnectOptions {
    /// The one `SET` a new connection runs before it is used.
    ///
    /// - `NO_BACKSLASH_ESCAPES` is always taken out of the session's SQL
    ///   mode: SQL rendered for MySQL writes a backslash in a string literal
    ///   as an escape, and a server whose global mode has it would read such
    ///   a literal differently, ending it early.
    /// - `PIPES_AS_CONCAT` makes `||` concatenate strings, not `OR` them.
    /// - `NO_ENGINE_SUBSTITUTION` makes a `CREATE TABLE` naming a missing
    ///   storage engine fail, not quietly use the default one.
    /// - `time_zone` is what `TIMESTAMP` values are read and written in
    ///   (UTC by default).
    /// - `SET NAMES` makes the server send and expect text in the chosen
    ///   character set, whatever each column stores; with no collation the
    ///   server picks that character set's default.
    ///
    /// The character set, collation and time zone are written into the
    /// statement, so a value that is not one is refused, never escaped.
    ///
    /// <https://dev.mysql.com/doc/refman/8.4/en/sql-mode.html>
    /// <https://dev.mysql.com/doc/refman/8.4/en/set-names.html>
    fn session_setup(&self) -> Result<String, Error> {
        let added: String = [
            (self.pipes_as_concat, ",PIPES_AS_CONCAT"),
            (self.no_engine_substitution, ",NO_ENGINE_SUBSTITUTION"),
        ]
        .into_iter()
        .filter_map(|(on, mode)| on.then_some(mode))
        .collect();

        // the server's own modes are kept, padded with commas so the removed
        // mode is matched as a whole name
        let mut assignments = vec![format!(
            "sql_mode=(SELECT TRIM(BOTH ',' FROM REPLACE(CONCAT(',', @@sql_mode, '{added},'), ',NO_BACKSLASH_ESCAPES,', ',')))"
        )];
        if let Some(timezone) = &self.timezone {
            let timezone = checked("time zone", timezone, |c| "_+-:/".contains(c))?;
            assignments.push(format!("time_zone='{timezone}'"));
        }
        if self.set_names {
            let charset = checked("character set", &self.charset, |c| c == '_')?;
            assignments.push(match &self.collation {
                Some(collation) => {
                    let collation = checked("collation", collation, |c| c == '_')?;
                    format!("NAMES {charset} COLLATE {collation}")
                }
                None => format!("NAMES {charset}"),
            });
        }

        Ok(format!("SET {};", assignments.join(",")))
    }
}

/// `value` when it is ASCII letters and digits plus what `also` allows.
fn checked<'a>(what: &str, value: &'a str, also: impl Fn(char) -> bool) -> Result<&'a str, Error> {
    if !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || also(c)) {
        Ok(value)
    } else {
        Err(Error::Configuration(
            format!("the MySQL {what} option is not a valid name").into(),
        ))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/options_connect_tests.rs"]
mod tests;
