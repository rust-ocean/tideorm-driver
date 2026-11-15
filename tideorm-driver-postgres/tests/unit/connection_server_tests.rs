use super::PgServer;

#[test]
fn the_reported_parameters_name_the_server() {
    let after = |parameters: &[(&str, &str)]| {
        parameters
            .iter()
            .fold(PgServer::Postgres, |server, (name, value)| {
                server.reported(name, value)
            })
    };
    assert_eq!(
        after(&[("server_version", "18.1 (Debian 18.1-1.pgdg13+2)")]),
        PgServer::Postgres
    );
    assert_eq!(
        after(&[("server_version", "15.12-YB-2025.2.8.1-b0")]),
        PgServer::YugabyteDb
    );
    for parameters in [
        &[
            ("server_version", "18.0.0"),
            ("crdb_version", "CockroachDB CCL v26.4.0"),
        ][..],
        &[
            ("crdb_version", "CockroachDB CCL v26.4.0"),
            ("server_version", "13.0.0"),
        ],
    ] {
        assert_eq!(after(parameters), PgServer::CockroachDb, "{parameters:?}");
    }
}
