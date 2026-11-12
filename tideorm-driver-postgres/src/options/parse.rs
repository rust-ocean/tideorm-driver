use crate::error::Error;
use crate::PgConnectOptions;
use driver_core::connection::decode_url_part;
use driver_core::percent_encoding::percent_decode_str;
use driver_core::Url;
use std::net::IpAddr;
use std::str::FromStr;

impl PgConnectOptions {
    pub(crate) fn parse_from_url(url: &Url) -> Result<Self, Error> {
        let mut options = Self::new_without_pgpass();

        if let Some(host) = url.host_str() {
            let host_decoded = percent_decode_str(host);
            options = match host_decoded.clone().next() {
                Some(b'/') => options.socket(&*host_decoded.decode_utf8().map_err(Error::config)?),
                _ => options.host(host),
            }
        }

        if let Some(port) = url.port() {
            options = options.port(port);
        }

        let username = url.username();
        if !username.is_empty() {
            options = options.username(&decode_url_part(username)?);
        }

        if let Some(password) = url.password() {
            options = options.password(&decode_url_part(password)?);
        }

        let path = url.path().trim_start_matches('/');
        if !path.is_empty() {
            options = options.database(&decode_url_part(path)?);
        }

        for (key, value) in url.query_pairs().into_iter() {
            match &*key {
                "sslmode" | "ssl-mode" => {
                    options = options.ssl_mode(value.parse().map_err(Error::config)?);
                }

                "sslrootcert" | "ssl-root-cert" | "ssl-ca" => {
                    options = options.ssl_root_cert(&*value);
                }

                "sslcert" | "ssl-cert" => options = options.ssl_client_cert(&*value),

                "sslkey" | "ssl-key" => options = options.ssl_client_key(&*value),

                "statement-cache-capacity" => {
                    options =
                        options.statement_cache_capacity(value.parse().map_err(Error::config)?);
                }

                "host" => {
                    if value.starts_with('/') {
                        options = options.socket(&*value);
                    } else {
                        options = options.host(&value);
                    }
                }

                "hostaddr" => {
                    value.parse::<IpAddr>().map_err(Error::config)?;
                    options = options.host(&value)
                }

                "port" => options = options.port(value.parse().map_err(Error::config)?),

                "dbname" => options = options.database(&value),

                "user" => options = options.username(&value),

                "password" => options = options.password(&value),

                "application_name" => options = options.application_name(&value),

                "options" => {
                    if let Some(options) = options.options.as_mut() {
                        options.push(' ');
                        options.push_str(&value);
                    } else {
                        options.options = Some(value.to_string());
                    }
                }

                k if k.starts_with("options[") => {
                    if let Some(key) = k.strip_prefix("options[").unwrap().strip_suffix(']') {
                        options = options.options([(key, &*value)]);
                    }
                }

                // the value may be a secret under a misspelled key
                _ => tracing::warn!(%key, "ignoring unrecognized connect parameter"),
            }
        }

        let options = options.apply_pgpass();

        Ok(options)
    }
}

impl FromStr for PgConnectOptions {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let url: Url = s.parse().map_err(Error::config)?;

        Self::parse_from_url(&url)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/options_parse_tests.rs"]
mod tests;
