use std::str::FromStr;

use driver_core::connection::decode_url_part;
use driver_core::Url;

use crate::error::Error;

use super::MySqlConnectOptions;

impl MySqlConnectOptions {
    pub(crate) fn parse_from_url(url: &Url) -> Result<Self, Error> {
        let mut options = Self::new();

        if let Some(host) = url.host_str() {
            options = options.host(host);
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

                "sslca" | "ssl-ca" => {
                    options = options.ssl_ca(&*value);
                }

                "charset" => {
                    options = options.charset(&value);
                }

                "collation" => {
                    options = options.collation(&value);
                }

                "sslcert" | "ssl-cert" => options = options.ssl_client_cert(&*value),

                "sslkey" | "ssl-key" => options = options.ssl_client_key(&*value),

                "statement-cache-capacity" => {
                    options =
                        options.statement_cache_capacity(value.parse().map_err(Error::config)?);
                }

                "socket" => {
                    options = options.socket(&*value);
                }

                "timezone" | "time-zone" => {
                    options = options.timezone(Some(value.to_string()));
                }

                _ => {}
            }
        }

        Ok(options)
    }
}

impl FromStr for MySqlConnectOptions {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let url: Url = s.parse().map_err(Error::config)?;
        Self::parse_from_url(&url)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/options_parse_tests.rs"]
mod tests;
