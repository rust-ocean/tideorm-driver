//! Loading SQLite extensions into each new connection.

use std::borrow::Cow;

use super::SqliteConnectOptions;

impl SqliteConnectOptions {
    /// Add a [SQLite extension](https://www.sqlite.org/loadext.html) to be loaded into the database
    /// connection at startup, using the default entrypoint.
    ///
    /// Most common SQLite extensions can be loaded using this method.
    /// For extensions where you need to override the entry point,
    /// use [`.extension_with_entrypoint()`].
    ///
    /// Multiple extensions can be loaded by calling this method,
    /// or [`.extension_with_entrypoint()`] where applicable,
    /// once for each extension.
    ///
    /// Extension loading is only enabled during the initialization of the connection,
    /// and disabled before `connect()` returns by setting
    /// [`SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION`] to 0.
    ///
    /// This will not enable the SQL `load_extension()` function.
    ///
    /// [`SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION`]: https://www.sqlite.org/c3ref/c_dbconfig_defensive.html#sqlitedbconfigenableloadextension
    /// [`.extension_with_entrypoint()`]: Self::extension_with_entrypoint
    ///
    /// # Safety
    /// This causes arbitrary DLLs on the filesystem to be loaded at runtime,
    /// which can easily result in undefined behavior, memory corruption,
    /// or exploitable vulnerabilities if misused.
    ///
    /// It is not possible to provide a truly safe version of this API.
    ///
    /// Use this method with care, and only load extensions that you trust.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use driver::Error;
    /// # use std::str::FromStr;
    /// # use driver::sqlite::SqliteConnectOptions;
    /// # fn options() -> Result<SqliteConnectOptions, Error> {
    /// let mut options = SqliteConnectOptions::from_str("sqlite://data.db")?;
    ///
    /// // SAFETY: these are trusted extensions.
    /// unsafe {
    ///     options = options
    ///         .extension("vsv")
    ///         .extension("mod_spatialite");
    /// }
    ///
    /// # Ok(options)
    /// # }
    /// ```
    pub unsafe fn extension(mut self, extension_name: impl Into<Cow<'static, str>>) -> Self {
        self.extensions.insert(extension_name.into(), None);
        self
    }

    /// Add a [SQLite extension](https://www.sqlite.org/loadext.html) to be loaded into the database
    /// connection at startup, overriding the entrypoint.
    ///
    /// See also [`.extension()`](Self::extension) for extensions using the standard entrypoint name
    /// `sqlite3_extension_init` or `sqlite3_<extension name>_init`.
    ///
    /// Multiple extensions can be loaded by calling this method,
    /// or [`.extension()`](Self::extension) where applicable,
    /// once for each extension.
    ///
    /// Extension loading is only enabled during the initialization of the connection,
    /// and disabled before `connect()` returns by setting
    /// [`SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION`] to 0.
    ///
    /// This will not enable the SQL `load_extension()` function.
    ///
    /// [`SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION`]: https://www.sqlite.org/c3ref/c_dbconfig_defensive.html#sqlitedbconfigenableloadextension
    /// [`.extension_with_entrypoint()`]: Self::extension_with_entrypoint
    ///
    /// # Safety
    /// This causes arbitrary DLLs on the filesystem to be loaded at runtime,
    /// which can easily result in undefined behavior, memory corruption,
    /// or exploitable vulnerabilities if misused.
    ///
    /// If you specify the wrong entrypoint name, it _may_ simply result in an error,
    /// or it may end up invoking the wrong routine, leading to undefined behavior.
    ///
    /// It is not possible to provide a truly safe version of this API.
    ///
    /// Use this method with care, only load extensions that you trust,
    /// and double-check the entrypoint name with the extension's documentation or source code.
    pub unsafe fn extension_with_entrypoint(
        mut self,
        extension_name: impl Into<Cow<'static, str>>,
        entry_point: impl Into<Cow<'static, str>>,
    ) -> Self {
        self.extensions
            .insert(extension_name.into(), Some(entry_point.into()));
        self
    }
}
