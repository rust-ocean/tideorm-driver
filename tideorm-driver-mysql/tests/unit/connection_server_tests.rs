use super::MySqlServer;

#[test]
fn the_handshake_version_names_the_server() {
    for (version, server) in [
        ("8.0.36", MySqlServer::MySql),
        ("8.4.3-commercial", MySqlServer::MySql),
        (
            "5.5.5-10.11.6-MariaDB-1:10.11.6+maria~ubu2204",
            MySqlServer::MariaDb,
        ),
        ("11.4.2-MariaDB-log", MySqlServer::MariaDb),
        ("8.0.11-TiDB-v8.5.8", MySqlServer::TiDb),
        ("5.7.25-TiDB-v6.5.0", MySqlServer::TiDb),
        ("8.0.43-Vitess", MySqlServer::Vitess),
        ("8.0.30-Vitess-Version 19.0.4", MySqlServer::Vitess),
    ] {
        assert_eq!(MySqlServer::from_version(version), server, "{version}");
    }
}
