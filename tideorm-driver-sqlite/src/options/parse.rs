use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use driver_core::connection::decode_url_part;

use crate::error::Error;
use crate::SqliteConnectOptions;

// https://www.sqlite.org/uri.html

static IN_MEMORY_DB_SEQ: AtomicUsize = AtomicUsize::new(0);

impl SqliteConnectOptions {
    pub(crate) fn from_db_and_params(database: &str, params: Option<&str>) -> Result<Self, Error> {
        let mut options = Self::default();

        if database == ":memory:" {
            options.in_memory = true;
            options.shared_cache = true;
            let seqno = IN_MEMORY_DB_SEQ.fetch_add(1, Ordering::Relaxed);
            options.filename = Cow::Owned(PathBuf::from(format!("file:tideorm-in-memory-{seqno}")));
        } else {
            // % decode to allow for `?` or `#` in the filename
            options.filename = Cow::Owned(Path::new(&*decode_url_part(database)?).to_path_buf());
        }

        if let Some(params) = params {
            for (key, value) in url::form_urlencoded::parse(params.as_bytes()) {
                match &*key {
                    // The mode query parameter determines if the new database is opened read-only,
                    // read-write, read-write and created if it does not exist, or that the
                    // database is a pure in-memory database that never interacts with disk,
                    // respectively. One mode applies: as in SQLite's own URIs, the last wins.
                    "mode" => {
                        match &*value {
                            "ro" => {
                                options.read_only = true;
                                options.create_if_missing = false;
                            }

                            // default
                            "rw" => {
                                options.read_only = false;
                                options.create_if_missing = false;
                            }

                            "rwc" => {
                                options.read_only = false;
                                options.create_if_missing = true;
                            }

                            "memory" => {
                                options.in_memory = true;
                                options.shared_cache = true;
                            }

                            _ => {
                                return Err(Error::Configuration(
                                    format!("unknown value {value:?} for `mode`").into(),
                                ));
                            }
                        }
                    }

                    // The cache query parameter specifies the cache behaviour across multiple
                    // connections to the same database within the process. A shared cache is
                    // essential for persisting data across connections to an in-memory database.
                    "cache" => match &*value {
                        "private" => {
                            options.shared_cache = false;
                        }

                        "shared" => {
                            options.shared_cache = true;
                        }

                        _ => {
                            return Err(Error::Configuration(
                                format!("unknown value {value:?} for `cache`").into(),
                            ));
                        }
                    },

                    "immutable" => match &*value {
                        "true" | "1" => {
                            options.immutable = true;
                        }
                        "false" | "0" => {
                            options.immutable = false;
                        }
                        _ => {
                            return Err(Error::Configuration(
                                format!("unknown value {value:?} for `immutable`").into(),
                            ));
                        }
                    },

                    "vfs" => options.vfs = Some(Cow::Owned(value.into_owned())),

                    _ => {
                        return Err(Error::Configuration(
                            format!("unknown query parameter `{key}` while parsing connection URL")
                                .into(),
                        ));
                    }
                }
            }
        }

        Ok(options)
    }
}

impl FromStr for SqliteConnectOptions {
    type Err = Error;

    fn from_str(mut url: &str) -> Result<Self, Self::Err> {
        // The scheme, in any case, and the `//` after it.
        if url
            .get(..7)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("sqlite:"))
        {
            url = &url[7..];
            url = url.strip_prefix("//").unwrap_or(url);
        }

        let mut database_and_params = url.splitn(2, '?');

        let database = database_and_params.next().unwrap_or_default();
        let params = database_and_params.next();

        Self::from_db_and_params(database, params)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/options_parse_tests.rs"]
mod tests;
