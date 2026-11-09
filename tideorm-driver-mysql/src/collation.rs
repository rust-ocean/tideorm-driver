/// A collation ID, as a column definition or the handshake carries it.
///
/// The driver needs no catalog of collations, which MySQL and MariaDB each
/// keep extending on their own: the server converts the text of every
/// non-binary column to the connection's character set (see `SET NAMES`)
/// before sending it, whatever the column stores. So the only distinction
/// that matters is [`BINARY`](Self::BINARY), whose bytes arrive as stored;
/// text in any other collation arrives in the connection's character set,
/// `utf8mb4` unless configured otherwise.
///
/// <https://dev.mysql.com/doc/refman/8.4/en/charset-connection.html>
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Collation(pub u16);

impl Collation {
    /// `binary`: the collation of byte strings and of every non-text value.
    pub const BINARY: Self = Collation(63);

    /// `utf8mb4_general_ci`, which every supported server has. The
    /// handshake asks for it, so it holds until a `SET NAMES`.
    pub const UTF8MB4_GENERAL_CI: Self = Collation(45);
}
