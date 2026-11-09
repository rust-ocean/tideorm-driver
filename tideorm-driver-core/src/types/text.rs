use std::ops::{Deref, DerefMut};

/// Map a SQL text value to/from a Rust type using [`Display`] and [`FromStr`].
///
/// This can be useful for types that do not have a direct SQL equivalent or are not
/// supported directly by the driver.
///
/// For strongly typed databases like Postgres, this will report the value's type as `TEXT`.
/// Explicit conversion may be necessary on the SQL side depending on the desired type.
///
/// [`Display`]: std::fmt::Display
/// [`FromStr`]: std::str::FromStr
///
/// ### Panics
///
/// You should only use this adapter with `Display` implementations that are infallible,
/// otherwise you may encounter panics when attempting to bind a value.
///
/// This is because the design of the `Encode` trait assumes encoding is infallible, so there is no
/// way to bubble up the error.
///
/// Fortunately, most `Display` implementations are infallible by convention anyway
/// (the standard `ToString` trait also assumes this), but you may still want to audit
/// the source code for any types you intend to use with this adapter, just to be safe.
///
/// ### Example: `SocketAddr`
///
/// MySQL and SQLite do not have a native SQL equivalent for `SocketAddr`, so if you want to
/// store and retrieve instances of it, it makes sense to map it to `TEXT`:
///
/// ```rust,no_run
/// use std::net::SocketAddr;
///
/// use driver::mysql::{MySqlConnection, MySqlRow};
/// use driver::types::{Text, chrono::NaiveDateTime, uuid::Uuid};
/// use driver::{Connection, Row};
///
/// #[derive(Debug)]
/// struct Login {
///     user_id: Uuid,
///     socket_addr: Text<SocketAddr>,
///     login_at: NaiveDateTime,
/// }
///
/// fn login(row: &MySqlRow) -> driver::Result<Login> {
///     Ok(Login {
///         user_id: row.try_get("user_id")?,
///         socket_addr: row.try_get("socket_addr")?,
///         login_at: row.try_get("login_at")?,
///     })
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut conn = MySqlConnection::connect("<DATABASE URL>").await?;
///
/// let user_id: Uuid = "e9a72cdc-d907-48d6-a488-c64a91fd063c".parse().unwrap();
/// let socket_addr: SocketAddr = "198.51.100.47:31790".parse().unwrap();
///
/// // CREATE TABLE user_login(user_id BINARY(16), socket_addr TEXT, login_at DATETIME);
/// driver::query("INSERT INTO user_login(user_id, socket_addr, login_at) VALUES (?, ?, NOW())")
///     .bind(user_id)
///     .bind(Text(socket_addr))
///     .execute(&mut conn)
///     .await?;
///
/// let logins = driver::query("SELECT * FROM user_login")
///     .fetch_all(&mut conn)
///     .await?
///     .iter()
///     .map(login)
///     .collect::<driver::Result<Vec<_>>>()?;
///
/// println!("Logins for user ID {user_id}: {logins:?}");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Text<T>(pub T);

impl<T> Text<T> {
    /// Extract the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Text<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Text<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
