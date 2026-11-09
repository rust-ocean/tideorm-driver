//! Read access to the settings of a `PgConnectOptions`.

use std::path::PathBuf;

use super::{PgConnectOptions, PgSslMode};

impl PgConnectOptions {
    /// Get the current host.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::PgConnectOptions;
    /// let options = PgConnectOptions::new()
    ///     .host("127.0.0.1");
    /// assert_eq!(options.get_host(), "127.0.0.1");
    /// ```
    pub fn get_host(&self) -> &str {
        &self.host
    }

    /// Get the server's port.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::PgConnectOptions;
    /// let options = PgConnectOptions::new()
    ///     .port(6543);
    /// assert_eq!(options.get_port(), 6543);
    /// ```
    pub fn get_port(&self) -> u16 {
        self.port
    }

    /// Get the socket path.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::PgConnectOptions;
    /// let options = PgConnectOptions::new()
    ///     .socket("/tmp");
    /// assert!(options.get_socket().is_some());
    /// ```
    pub fn get_socket(&self) -> Option<&PathBuf> {
        self.socket.as_ref()
    }

    /// Get the server's port.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::PgConnectOptions;
    /// let options = PgConnectOptions::new()
    ///     .username("foo");
    /// assert_eq!(options.get_username(), "foo");
    /// ```
    pub fn get_username(&self) -> &str {
        &self.username
    }

    /// Get the password, if one is set.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::PgConnectOptions;
    /// let options: PgConnectOptions = "postgres://app:p%40ss@localhost/app".parse().unwrap();
    /// assert_eq!(options.get_password(), Some("p@ss"));
    /// ```
    pub fn get_password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// Get the current database name.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::PgConnectOptions;
    /// let options = PgConnectOptions::new()
    ///     .database("postgres");
    /// assert!(options.get_database().is_some());
    /// ```
    pub fn get_database(&self) -> Option<&str> {
        self.database.as_deref()
    }

    /// Get the SSL mode.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::{PgConnectOptions, PgSslMode};
    /// let options = PgConnectOptions::new();
    /// assert!(matches!(options.get_ssl_mode(), PgSslMode::Prefer));
    /// ```
    pub fn get_ssl_mode(&self) -> PgSslMode {
        self.ssl_mode
    }

    /// Get the application name.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::PgConnectOptions;
    /// let options = PgConnectOptions::new()
    ///     .application_name("service");
    /// assert!(options.get_application_name().is_some());
    /// ```
    pub fn get_application_name(&self) -> Option<&str> {
        self.application_name.as_deref()
    }

    /// Get the options.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use driver::postgres::PgConnectOptions;
    /// let options = PgConnectOptions::new()
    ///     .options([("foo", "bar")]);
    /// assert!(options.get_options().is_some());
    /// ```
    pub fn get_options(&self) -> Option<&str> {
        self.options.as_deref()
    }
}
