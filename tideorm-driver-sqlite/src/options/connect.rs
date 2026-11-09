//! [`ConnectOptions`] for SQLite: open the database, then apply the configured pragmas and
//! collations to the new connection.

use crate::{SqliteConnectOptions, SqliteConnection};
use driver_core::connection::ConnectOptions;
use driver_core::error::Error;
use driver_core::executor::Executor;
use driver_core::sql_str::AssertSqlSafe;
use std::fmt::Write;
use std::str::FromStr;
use url::Url;

impl ConnectOptions for SqliteConnectOptions {
    type Connection = SqliteConnection;

    fn from_url(url: &Url) -> Result<Self, Error> {
        // Parse the URL's text, not its parts: the URL parser reads `foo.db` in
        // `sqlite://foo.db` as a host and leaves the path empty, yet it names the same file
        // as `sqlite:foo.db`.
        Self::from_str(url.as_str())
    }

    async fn connect(&self) -> Result<Self::Connection, Error>
    where
        Self::Connection: Sized,
    {
        let mut conn = SqliteConnection::establish(self).await?;

        conn.execute(AssertSqlSafe(self.pragma_string())).await?;

        // Registering a collation takes the raw handle, so lock it only when there are some.
        if !self.collations.is_empty() {
            let mut locked = conn.lock_handle().await?;

            for collation in &self.collations {
                collation.create(&mut locked.guard.handle)?;
            }
        }

        Ok(conn)
    }
}

impl SqliteConnectOptions {
    /// One batch of `PRAGMA key = value;` statements, in the order the pragmas were first
    /// set; a pragma whose value is `None` keeps a place in that order but is not sent.
    pub(crate) fn pragma_string(&self) -> String {
        let mut string = String::new();

        for (key, opt_value) in &self.pragmas {
            if let Some(value) = opt_value {
                write!(string, "PRAGMA {key} = {value}; ").ok();
            }
        }

        string
    }
}
