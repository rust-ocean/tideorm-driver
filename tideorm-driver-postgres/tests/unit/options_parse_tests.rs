use super::*;

#[test]
fn it_parses_socket_correctly_from_parameter() {
    let url = "postgres:///?host=/var/run/postgres/";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(Some("/var/run/postgres/".into()), opts.socket);
}

#[test]
fn it_parses_host_correctly_from_parameter() {
    let url = "postgres:///?host=google.database.com";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(None, opts.socket);
    assert_eq!("google.database.com", &opts.host);
}

#[test]
fn it_parses_hostaddr_correctly_from_parameter() {
    let url = "postgres:///?hostaddr=8.8.8.8";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(None, opts.socket);
    assert_eq!("8.8.8.8", &opts.host);
}

#[test]
fn it_parses_port_correctly_from_parameter() {
    let url = "postgres:///?port=1234";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(None, opts.socket);
    assert_eq!(1234, opts.port);
}

#[test]
fn it_parses_dbname_correctly_from_parameter() {
    let url = "postgres:///?dbname=some_db";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(None, opts.socket);
    assert_eq!(Some("some_db"), opts.database.as_deref());
}

#[test]
fn it_parses_user_correctly_from_parameter() {
    let url = "postgres:///?user=some_user";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(None, opts.socket);
    assert_eq!("some_user", opts.username);
}

#[test]
fn it_parses_password_correctly_from_parameter() {
    let url = "postgres:///?password=some_pass";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(None, opts.socket);
    assert_eq!(Some("some_pass"), opts.password.as_deref());
}

#[test]
fn it_parses_application_name_correctly_from_parameter() {
    let url = "postgres:///?application_name=some_name";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(Some("some_name"), opts.application_name.as_deref());
}

#[test]
fn it_parses_socket_correctly_percent_encoded() {
    let url = "postgres://%2Fvar%2Flib%2Fpostgres/database";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(Some("/var/lib/postgres/".into()), opts.socket);
}

#[test]
fn it_parses_socket_correctly_with_username_percent_encoded() {
    let url = "postgres://some_user@%2Fvar%2Flib%2Fpostgres/database";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!("some_user", opts.username);
    assert_eq!(Some("/var/lib/postgres/".into()), opts.socket);
    assert_eq!(Some("database"), opts.database.as_deref());
}

#[test]
fn it_parses_libpq_options_correctly() {
    let url = "postgres:///?options=-c%20synchronous_commit%3Doff%20--search_path%3Dpostgres";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(
        Some("-c synchronous_commit=off --search_path=postgres".into()),
        opts.options
    );
}

#[test]
fn it_parses_options_correctly() {
    let url = "postgres:///?options[synchronous_commit]=off&options[search_path]=postgres";
    let opts = PgConnectOptions::from_str(url).unwrap();

    assert_eq!(
        Some("-c synchronous_commit=off -c search_path=postgres".into()),
        opts.options
    );
}
