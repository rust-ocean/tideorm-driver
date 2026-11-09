use std::path::PathBuf;

use crate::error::Error;
use crate::net::socket::WithSocket;
use crate::net::Socket;

#[cfg(feature = "_tls-rustls")]
mod tls_rustls;

#[cfg(feature = "_tls-rustls")]
mod util;

/// X.509 Certificate input, either a file path or a PEM encoded inline certificate(s).
#[derive(Clone, Debug)]
pub enum CertificateInput {
    /// PEM encoded certificate(s)
    Inline(Vec<u8>),
    /// Path to a file containing PEM encoded certificate(s)
    File(PathBuf),
}

impl From<String> for CertificateInput {
    fn from(value: String) -> Self {
        // Leading and trailing whitespace/newlines
        let trimmed = value.trim();

        // Heuristic for PEM encoded inputs:
        // https://tools.ietf.org/html/rfc7468
        if trimmed.starts_with("-----BEGIN") && trimmed.ends_with("-----") {
            CertificateInput::Inline(value.as_bytes().to_vec())
        } else {
            CertificateInput::File(PathBuf::from(value))
        }
    }
}

impl CertificateInput {
    #[cfg(feature = "_tls-rustls")]
    async fn data(&self) -> Result<Vec<u8>, std::io::Error> {
        use crate::fs;
        match self {
            CertificateInput::Inline(v) => Ok(v.clone()),
            CertificateInput::File(path) => fs::read(path).await,
        }
    }
}

impl std::fmt::Display for CertificateInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CertificateInput::Inline(v) => write!(f, "{}", String::from_utf8_lossy(v.as_slice())),
            CertificateInput::File(path) => write!(f, "file: {}", path.display()),
        }
    }
}

pub struct TlsConfig<'a> {
    pub accept_invalid_certs: bool,
    pub accept_invalid_hostnames: bool,
    pub hostname: &'a str,
    pub root_cert_path: Option<&'a CertificateInput>,
    pub client_cert_path: Option<&'a CertificateInput>,
    pub client_key_path: Option<&'a CertificateInput>,
}

pub async fn handshake<S, Ws>(
    socket: S,
    config: TlsConfig<'_>,
    with_socket: Ws,
) -> crate::Result<Ws::Output>
where
    S: Socket,
    Ws: WithSocket,
{
    #[cfg(feature = "_tls-rustls")]
    return Ok(with_socket
        .with_socket(tls_rustls::handshake(socket, config).await?)
        .await);

    #[cfg(not(feature = "_tls-rustls"))]
    {
        drop((socket, config, with_socket));
        panic!("the `tls-rustls` feature of `tideorm-driver` must be enabled")
    }
}

pub fn available() -> bool {
    cfg!(feature = "_tls-rustls")
}

pub fn error_if_unavailable() -> crate::Result<()> {
    if !available() {
        return Err(Error::tls(
            "TLS upgrade required by connect options, but the driver was built without TLS \
                    support; enable the `tls-rustls` feature of `tideorm-driver`",
        ));
    }

    Ok(())
}
