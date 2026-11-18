use super::*;

/// A cached statement is reused only for arguments of its parameters' types: one parsed
/// for an `INT4` would read an `INT8`'s eight bytes as a malformed `INT4`.
#[test]
fn a_statement_takes_only_its_own_parameter_types() {
    type T = PgTypeInfo;

    assert!(takes_types(&[T::INT4], &[T::INT4]));
    assert!(!takes_types(&[T::INT4], &[T::INT8]));
    assert!(!takes_types(&[T::INT8, T::INT4], &[T::INT8, T::INT8]));
    // a type declared by name has no OID until resolved; patching its holes checks it
    assert!(takes_types(&[T::INT4], &[T::with_name("mood")]));
    assert!(takes_types(&[], &[]));
}

/// A server for the extended protocol that serves one session and logs what it parses, binds and
/// closes. A bind of the session's first statement after its first one fails as PostgreSQL
/// fails a cached plan whose table changed its columns.
fn changing_table_server() -> (String, std::thread::JoinHandle<Vec<String>>) {
    use crate::test_support::{read_body, send};
    use std::io::Read;

    fn name(body: &[u8]) -> String {
        let end = body.iter().position(|&b| b == 0).unwrap();
        String::from_utf8(body[..end].to_vec()).unwrap()
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "postgres://test@{}/test?sslmode=disable",
        listener.local_addr().unwrap()
    );
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_body(&mut stream).unwrap();
        send(&mut stream, b'R', &0_u32.to_be_bytes()).unwrap();
        send(&mut stream, b'Z', b"I").unwrap();
        let (mut log, mut first, mut binds_of_first, mut failed) = (Vec::new(), None, 0, false);
        loop {
            let mut tag = [0];
            if stream.read_exact(&mut tag).is_err() {
                return log;
            }
            let body = read_body(&mut stream).unwrap();
            if failed && tag[0] != b'S' {
                continue; // after an error the server skips everything up to the Sync
            }
            match tag[0] {
                b'P' => {
                    log.push(format!("parse {}", name(&body)));
                    first.get_or_insert_with(|| name(&body));
                    send(&mut stream, b'1', b"").unwrap();
                }
                b'D' => {
                    send(&mut stream, b't', &0_u16.to_be_bytes()).unwrap();
                    let mut row = 1_u16.to_be_bytes().to_vec();
                    row.extend(b"a\0");
                    row.extend([0; 6]); // table and column number
                    row.extend(23_u32.to_be_bytes()); // INT4
                    row.extend(4_i16.to_be_bytes());
                    row.extend((-1_i32).to_be_bytes());
                    row.extend(1_i16.to_be_bytes()); // binary
                    send(&mut stream, b'T', &row).unwrap();
                }
                b'B' => {
                    let statement = name(&body[body.iter().position(|&b| b == 0).unwrap() + 1..]);
                    log.push(format!("bind {statement}"));
                    if first.as_ref() == Some(&statement) {
                        binds_of_first += 1;
                        if binds_of_first > 1 {
                            failed = true;
                            send(
                                &mut stream,
                                b'E',
                                b"SERROR\0C0A000\0Mcached plan must not change result type\0\0",
                            )
                            .unwrap();
                            continue;
                        }
                    }
                    send(&mut stream, b'2', b"").unwrap();
                }
                b'E' => {
                    let mut row = 1_u16.to_be_bytes().to_vec();
                    row.extend(4_u32.to_be_bytes());
                    row.extend(7_i32.to_be_bytes());
                    send(&mut stream, b'D', &row).unwrap();
                    send(&mut stream, b'C', b"SELECT 1\0").unwrap();
                }
                b'C' => {
                    if body[0] == b'S' {
                        log.push(format!("close {}", name(&body[1..])));
                    }
                    send(&mut stream, b'3', b"").unwrap();
                }
                b'S' => {
                    failed = false;
                    send(&mut stream, b'Z', b"I").unwrap();
                }
                b'X' => return log,
                other => panic!("unexpected message {:?}", other as char),
            }
        }
    });
    (url, server)
}

/// A table change that alters a cached statement's result columns fails that statement's run;
/// the statement is then parsed again, not refused on every later run of the connection.
#[test]
fn a_statement_whose_plan_went_stale_is_parsed_again() {
    use driver_core::connection::Connection;
    use driver_core::row::Row;

    let (url, server) = changing_table_server();
    driver_core::rt::test_block_on(async {
        let mut connection = PgConnection::connect(&url).await.unwrap();
        let read = || driver_core::query::query("SELECT a FROM t");
        let row = read().fetch_one(&mut connection).await.unwrap();
        assert_eq!(row.get::<i32, _>(0), 7);

        let error = read().fetch_one(&mut connection).await.unwrap_err();
        assert!(is_stale_plan(&error), "{error:?}");

        for _ in 0..2 {
            let row = read().fetch_one(&mut connection).await.unwrap();
            assert_eq!(row.get::<i32, _>(0), 7);
        }
        connection.close().await.unwrap();
    });
    let log = server.join().unwrap();
    let first = log[0].strip_prefix("parse ").unwrap();
    let second = log[4].strip_prefix("parse ").unwrap();
    assert_ne!(first, second);
    assert_eq!(
        log,
        [
            format!("parse {first}"),
            format!("bind {first}"),
            format!("bind {first}"),
            format!("close {first}"),
            format!("parse {second}"),
            format!("bind {second}"),
            format!("bind {second}"),
        ]
    );
}

/// A reply no one awaits any longer (a dropped stream's, a queued rollback's) is read before the
/// next statement; an error in it ended that statement and is not the next one's.
#[test]
fn an_error_in_an_abandoned_reply_does_not_fail_the_next_statement() {
    use crate::test_support::{Answer, FakeServer};
    use driver_core::connection::Connection;
    use driver_core::executor::Executor;

    let server = FakeServer::start(|sql| match sql {
        "abandoned" => Answer::Fail,
        _ => Answer::Done,
    });
    driver_core::rt::test_block_on(async {
        let mut connection = PgConnection::connect(&server.url).await.unwrap();
        connection.queue_simple_query("abandoned").unwrap();
        connection.execute("next").await.unwrap();
        connection.close().await.unwrap();
    });
    assert_eq!(server.queries(), ["1:abandoned", "1:next"]);
}
