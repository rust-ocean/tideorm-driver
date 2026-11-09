bitflags::bitflags! {
    /// What the client and the server each support. The server lists its
    /// flags in its greeting; the client answers with the ones it uses, and
    /// both follow those for the rest of the connection.
    ///
    /// MySQL's flags fill 32 bits. MariaDB sends 32 more in bytes MySQL
    /// reserves, so this type holds both halves: the high half is MariaDB's.
    ///
    /// <https://dev.mysql.com/doc/dev/mysql-server/latest/group__group__cs__capabilities__flags.html>
    /// <https://mariadb.com/kb/en/connection/#capabilities>
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Capabilities: u64 {
        /// Set by MySQL (its `CLIENT_LONG_PASSWORD`). A MariaDB server
        /// leaves it clear to say its extended flags follow.
        const MYSQL = 1;

        /// Affected-row counts count the rows matched, changed or not.
        const FOUND_ROWS = 2;

        /// Column definitions carry all their flags.
        const LONG_FLAG = 4;

        /// The handshake response names the database to start in.
        const CONNECT_WITH_DB = 8;

        /// `database.table.column` names are refused.
        const NO_SCHEMA = 16;

        /// Packets are compressed.
        const COMPRESS = 32;

        /// Obsolete ODBC behaviour.
        const ODBC = 64;

        /// `LOAD DATA LOCAL` may read the client's files.
        const LOCAL_FILES = 128;

        /// A function name may have spaces before its `(`.
        const IGNORE_SPACE = 256;

        /// The 4.1 protocol, which every server the driver supports speaks.
        const PROTOCOL_41 = 512;

        /// The session times out after `interactive_timeout`, not
        /// `wait_timeout`.
        const INTERACTIVE = 1024;

        /// The connection switches to TLS before the client authenticates.
        const SSL = 2048;

        /// OK and EOF packets carry the server's status flags.
        const TRANSACTIONS = 8192;

        /// The 4.1 authentication, whose response is preceded by its length.
        const SECURE_CONNECTION = 1 << 15;

        /// A `COM_QUERY` or `COM_STMT_PREPARE` may hold several statements.
        const MULTI_STATEMENTS = 1 << 16;

        /// A `COM_QUERY` may return several results.
        const MULTI_RESULTS = 1 << 17;

        /// An execute may return several results (a stored procedure's).
        const PS_MULTI_RESULTS = 1 << 18;

        /// The handshake names the authentication plugin.
        const PLUGIN_AUTH = 1 << 19;

        /// The handshake response carries connection attributes.
        const CONNECT_ATTRS = 1 << 20;

        /// The authentication response is length-encoded, so it may pass
        /// 255 bytes.
        const PLUGIN_AUTH_LENENC_DATA = 1 << 21;

        /// An account whose password expired may still log in, to change it.
        const CAN_HANDLE_EXPIRED_PASSWORDS = 1 << 22;

        /// OK packets report changes to the session's state.
        const SESSION_TRACK = 1 << 23;

        /// An OK packet ends a result set, in place of an EOF packet.
        const DEPRECATE_EOF = 1 << 24;

        /// Packets may be compressed with zstd.
        const ZSTD_COMPRESSION_ALGORITHM = 1 << 26;

        /// The client checks the server's certificate; never sent.
        const SSL_VERIFY_SERVER_CERT = 1 << 30;

        /// A result set may leave out its column definitions.
        const OPTIONAL_RESULTSET_METADATA = 1 << 25;

        /// The client keeps its options after a failed connect; never sent.
        const REMEMBER_OPTIONS = 1 << 31;

        /// MariaDB: progress reports for long statements.
        const MARIADB_CLIENT_PROGRESS = 1 << 32;

        /// MariaDB: `COM_MULTI`, several commands in one packet.
        const MARIADB_CLIENT_MULTI = 1 << 33;

        /// MariaDB: bulk executes of a prepared statement.
        const MARIADB_CLIENT_STMT_BULK_OPERATIONS = 1 << 34;

        /// MariaDB: column definitions name extended types, such as `json`
        /// and `uuid`.
        const MARIADB_CLIENT_EXTENDED_TYPE_INFO = 1 << 35;

        /// MariaDB: an execute may skip the column definitions the client
        /// already holds.
        const MARIADB_CLIENT_CACHE_METADATA = 1 << 36;

        /// MariaDB: a bulk execute may return one result per row, with its
        /// affected rows and insert ID.
        const MARIADB_CLIENT_BULK_UNIT_RESULTS = 1 << 37;
    }
}
