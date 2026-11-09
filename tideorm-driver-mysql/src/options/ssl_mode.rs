use crate::error::Error;
use std::str::FromStr;

/// Whether the connection uses TLS, and how far it trusts the server's
/// certificate.
///
/// Set it with [`ssl_mode`](super::MySqlConnectOptions::ssl_mode), or with
/// `ssl-mode` in a connection URL, which takes the names MySQL's own clients
/// use (`DISABLED`, `PREFERRED`, `REQUIRED`, `VERIFY_CA`, `VERIFY_IDENTITY`)
/// in any case.
///
/// <https://dev.mysql.com/doc/refman/8.4/en/connection-options.html#option_general_ssl-mode>
#[derive(Debug, Clone, Copy, Default)]
pub enum MySqlSslMode {
    /// Never TLS.
    Disabled,

    /// TLS when the server offers it and the driver was built with TLS
    /// support, an unencrypted connection otherwise. The default.
    #[default]
    Preferred,

    /// TLS, or no connection. The server's certificate is not checked.
    Required,

    /// As `Required`, and the server's certificate must be signed by a
    /// trusted certificate authority (see `ssl_ca`).
    VerifyCa,

    /// As `VerifyCa`, and the certificate must also name the host connected
    /// to.
    VerifyIdentity,
}

impl FromStr for MySqlSslMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        const NAMES: [(&str, MySqlSslMode); 5] = [
            ("disabled", MySqlSslMode::Disabled),
            ("preferred", MySqlSslMode::Preferred),
            ("required", MySqlSslMode::Required),
            ("verify_ca", MySqlSslMode::VerifyCa),
            ("verify_identity", MySqlSslMode::VerifyIdentity),
        ];

        NAMES
            .into_iter()
            .find_map(|(name, mode)| name.eq_ignore_ascii_case(s).then_some(mode))
            .ok_or_else(|| {
                Error::Configuration(format!("unknown value {s:?} for `ssl_mode`").into())
            })
    }
}
