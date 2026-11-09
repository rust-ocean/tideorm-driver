bitflags::bitflags! {
    /// The server's state after a statement, sent in OK and EOF packets. The
    /// driver follows it to know whether a transaction is open and whether
    /// another result follows.
    ///
    /// <https://dev.mysql.com/doc/dev/mysql-server/latest/mysql__com_8h.html>
    /// <https://mariadb.com/kb/en/ok_packet/#server-status-flag>
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub struct Status: u16 {
        /// A transaction is open, begun by `BEGIN` or by a statement run
        /// with autocommit off.
        const SERVER_STATUS_IN_TRANS = 1;

        /// Autocommit is on.
        const SERVER_STATUS_AUTOCOMMIT = 2;

        /// Another result follows this one.
        const SERVER_MORE_RESULTS_EXISTS = 8;

        /// The statement found no good index to use.
        const SERVER_QUERY_NO_GOOD_INDEX_USED = 16;

        /// The statement used no index.
        const SERVER_QUERY_NO_INDEX_USED = 32;

        /// A cursor has rows left for `COM_STMT_FETCH`.
        const SERVER_STATUS_CURSOR_EXISTS = 64;

        /// A cursor has sent its last row.
        const SERVER_STATUS_LAST_ROW_SENT = 128;

        /// A database was dropped.
        const SERVER_STATUS_DB_DROPPED = (1 << 8);

        /// The SQL mode has `NO_BACKSLASH_ESCAPES`: a backslash in a string
        /// literal is an ordinary character.
        const SERVER_STATUS_NO_BACKSLASH_ESCAPES = (1 << 9);

        /// A prepared statement was prepared again, because a schema change
        /// changed its result columns.
        const SERVER_STATUS_METADATA_CHANGED = (1 << 10);

        /// The statement ran longer than `long_query_time`.
        const SERVER_QUERY_WAS_SLOW = (1 << 11);

        /// This result holds a stored procedure's `OUT` parameters.
        const SERVER_PS_OUT_PARAMS = (1 << 12);

        /// The open transaction is read-only.
        const SERVER_STATUS_IN_TRANS_READONLY = (1 << 13);

        /// The statement changed the session's state; the OK packet says how.
        const SERVER_SESSION_STATE_CHANGED = (1 << 14);
    }
}
