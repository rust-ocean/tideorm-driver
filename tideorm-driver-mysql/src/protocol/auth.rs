use std::str::FromStr;

use crate::error::Error;

/// The authentication plugins the driver can answer. The server names the
/// plugin in its greeting or in a later switch request.
///
/// <https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase_authentication_methods.html>
#[derive(Debug, Copy, Clone)]
// Each variant spells its plugin's name, and they all end in "Password".
#[allow(clippy::enum_variant_names)]
pub enum AuthPlugin {
    MySqlNativePassword,
    CachingSha2Password,
    Sha256Password,
    MySqlClearPassword,
}

impl AuthPlugin {
    const ALL: [AuthPlugin; 4] = [
        AuthPlugin::MySqlNativePassword,
        AuthPlugin::CachingSha2Password,
        AuthPlugin::Sha256Password,
        AuthPlugin::MySqlClearPassword,
    ];

    /// The plugin's name on the wire.
    pub(crate) fn name(self) -> &'static str {
        match self {
            AuthPlugin::MySqlNativePassword => "mysql_native_password",
            AuthPlugin::CachingSha2Password => "caching_sha2_password",
            AuthPlugin::Sha256Password => "sha256_password",
            AuthPlugin::MySqlClearPassword => "mysql_clear_password",
        }
    }
}

impl FromStr for AuthPlugin {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|plugin| plugin.name() == s)
            .ok_or_else(|| err_protocol!("unknown authentication plugin: {}", s))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/protocol_auth_tests.rs"]
mod tests;
