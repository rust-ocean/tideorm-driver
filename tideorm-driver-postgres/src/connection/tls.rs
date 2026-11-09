use crate::error::Error;
use crate::net::tls::{self, TlsConfig};
use crate::net::{Socket, SocketIntoBox, WithSocket};

use crate::message::SslRequest;
use crate::{PgConnectOptions, PgSslMode};

/// Wraps a freshly opened socket in TLS when the options' `sslmode` asks for it and the server
/// agrees.
pub struct MaybeUpgradeTls<'a>(pub &'a PgConnectOptions);

impl WithSocket for MaybeUpgradeTls<'_> {
    type Output = crate::Result<Box<dyn Socket>>;

    async fn with_socket<S: Socket>(self, socket: S) -> Self::Output {
        maybe_upgrade(socket, self.0).await
    }
}

/// Applies `sslmode` as libpq does:
/// <https://www.postgresql.org/docs/current/libpq-ssl.html#LIBPQ-SSL-SSLMODE-STATEMENTS>.
async fn maybe_upgrade<S: Socket>(
    mut socket: S,
    options: &PgConnectOptions,
) -> Result<Box<dyn Socket>, Error> {
    let encrypted = match options.ssl_mode {
        PgSslMode::Disable | PgSslMode::Allow => false,
        // TLS if both sides can, plain text otherwise
        PgSslMode::Prefer => tls::available() && request_tls(&mut socket).await?,
        PgSslMode::Require | PgSslMode::VerifyCa | PgSslMode::VerifyFull => {
            tls::error_if_unavailable()?;
            if !request_tls(&mut socket).await? {
                return Err(Error::Tls("server does not support TLS".into()));
            }
            true
        }
    };

    if !encrypted {
        return Ok(Box::new(socket));
    }

    // `verify-ca` checks the certificate chain, and `verify-full` the host name as well.
    let config = TlsConfig {
        accept_invalid_certs: !matches!(
            options.ssl_mode,
            PgSslMode::VerifyCa | PgSslMode::VerifyFull
        ),
        accept_invalid_hostnames: !matches!(options.ssl_mode, PgSslMode::VerifyFull),
        hostname: &options.host,
        root_cert_path: options.ssl_root_cert.as_ref(),
        client_cert_path: options.ssl_client_cert.as_ref(),
        client_key_path: options.ssl_client_key.as_ref(),
    };

    tls::handshake(socket, config, SocketIntoBox).await
}

/// Sends an `SSLRequest`, in place of the start-up message, and reads the server's one-byte
/// answer: `S` if it will speak TLS, `N` if it will not.
/// <https://www.postgresql.org/docs/current/protocol-flow.html#PROTOCOL-FLOW-SSL>
async fn request_tls(socket: &mut impl Socket) -> Result<bool, Error> {
    socket.write(SslRequest::BYTES).await?;

    let mut answer = [0u8];
    socket.read(&mut &mut answer[..]).await?;

    match answer[0] {
        b'S' => Ok(true),
        b'N' => Ok(false),
        other => Err(err_protocol!(
            "unexpected response from SSLRequest: 0x{:02x}",
            other
        )),
    }
}
