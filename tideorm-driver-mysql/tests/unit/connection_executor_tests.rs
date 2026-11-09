use super::*;
use crate::common::StatementCache;
use crate::connection::MySqlConnectionInner;
use crate::test_support::packet;
use crate::MySqlConnectOptions;
use driver_core::net::ScriptedSocket;
use driver_core::sql_str::SqlSafeStr;
use futures_util::FutureExt;
use std::sync::Mutex;

/// A connection with a statement cache of `capacity`, reading what the test
/// puts in the returned input.
fn connection(capacity: usize) -> (MySqlConnection, Arc<Mutex<Vec<u8>>>) {
    let socket = ScriptedSocket::new(Vec::new());
    let input = socket.input.clone();
    let stream = MySqlStream::with_socket(&MySqlConnectOptions::new(), socket).boxed_socket();
    let connection = MySqlConnection {
        inner: Box::new(MySqlConnectionInner {
            stream,
            transaction_depth: 0,
            cache_statement: StatementCache::new(capacity),
            mariadb: false,
        }),
    };
    (connection, input)
}

/// The `PrepareOk` of statement 7, with `params` parameters and no columns.
fn prepare_ok(params: u16) -> Vec<u8> {
    let mut payload = vec![0];
    payload.extend(7_u32.to_le_bytes());
    payload.extend(0_u16.to_le_bytes());
    payload.extend(params.to_le_bytes());
    payload.extend([0, 0, 0]);
    packet(&payload)
}

/// A parameter's definition, with MariaDB's extended type info (empty),
/// which the stream's capabilities ask for.
fn definition() -> Vec<u8> {
    let mut payload = Vec::new();
    for name in ["def", "", "", "", "?", "", ""] {
        payload.push(u8::try_from(name.len()).unwrap());
        payload.extend(name.as_bytes());
    }
    payload.push(0x0c);
    payload.extend(63_u16.to_le_bytes());
    payload.extend(0_u32.to_le_bytes());
    payload.push(0xfd); // VAR_STRING
    payload.extend(0_u16.to_le_bytes());
    payload.extend([0, 0, 0]);
    packet(&payload)
}

fn one_value() -> MySqlArguments {
    let mut arguments = MySqlArguments::default();
    arguments.add(1_i32).unwrap();
    arguments
}

/// A statement given the wrong number of values fails before its execute is
/// sent, and leaves no reply awaited: the connection goes on. An uncached
/// statement is closed on the server with the next flush.
#[test]
fn a_wrong_value_count_leaves_no_reply_awaited() {
    driver_core::rt::test_block_on(async {
        for capacity in [0, 10] {
            let (mut conn, input) = connection(capacity);
            input
                .lock()
                .unwrap()
                .extend([prepare_ok(2), definition(), definition()].concat());
            let error = {
                let stream = conn
                    .run("SELECT ?, ?".into_sql_str(), Some(one_value()), true)
                    .await
                    .unwrap();
                let mut stream = pin!(stream);
                stream.try_next().await.unwrap_err()
            };
            assert!(
                error.to_string().contains("expected 2 parameters"),
                "{error}"
            );
            assert!(conn.inner.stream.waiting.is_empty());
            let close = if capacity == 0 {
                packet(&[0x19, 7, 0, 0, 0])
            } else {
                Vec::new()
            };
            assert_eq!(conn.inner.stream.write_buffer().get(), close);
            conn.inner
                .stream
                .wait_until_ready()
                .now_or_never()
                .expect("no reply is awaited")
                .unwrap();
        }
    });
}

/// A statement cancelled before its prepare's response is read whole leaves
/// the connection refused, so a pool closes it instead of reading the rest of
/// that response as the next statement's reply. Cancelled once the prepare is
/// read, it leaves the execute's reply to be read first.
#[test]
fn a_prepare_cancelled_part_way_refuses_the_connection() {
    driver_core::rt::test_block_on(async {
        let response = [prepare_ok(1), definition()];
        for read in 0..=response.len() {
            let (mut conn, input) = connection(10);
            input.lock().unwrap().extend(response[..read].concat());
            {
                let stream = conn
                    .run("SELECT ?".into_sql_str(), Some(one_value()), true)
                    .await
                    .unwrap();
                let mut stream = pin!(stream);
                assert!(stream.try_next().now_or_never().is_none());
            }
            let ready = conn.inner.stream.wait_until_ready().now_or_never();
            if read < response.len() {
                let refused = ready.expect("a refusal waits for nothing").unwrap_err();
                assert!(
                    refused.to_string().contains("cancelled while its prepare"),
                    "{refused}"
                );
            } else {
                assert!(ready.is_none(), "the execute's reply is awaited");
                input.lock().unwrap().extend(packet(&[0, 0, 0, 2, 0, 0, 0]));
                conn.inner.stream.wait_until_ready().await.unwrap();
                assert!(conn.inner.stream.waiting.is_empty());
            }
        }
    });
}

/// A prepare the server refuses is answered by its ERR packet alone, which
/// leaves the connection ready.
#[test]
fn a_refused_prepare_leaves_the_connection_ready() {
    driver_core::rt::test_block_on(async {
        let (mut conn, input) = connection(10);
        input
            .lock()
            .unwrap()
            .extend(packet(b"\xff\x28\x04#42000You have an error"));
        let error = {
            let stream = conn
                .run("SELEC ?".into_sql_str(), Some(one_value()), true)
                .await
                .unwrap();
            let mut stream = pin!(stream);
            stream.try_next().await.unwrap_err()
        };
        assert!(error.as_database_error().is_some(), "{error}");
        assert!(conn.inner.stream.waiting.is_empty());
        conn.inner
            .stream
            .wait_until_ready()
            .now_or_never()
            .unwrap()
            .unwrap();
    });
}

/// A ping cancelled before its OK arrives leaves that OK awaited, to be read
/// before the next statement's reply.
#[test]
fn a_cancelled_ping_leaves_its_reply_awaited() {
    use driver_core::connection::Connection;
    driver_core::rt::test_block_on(async {
        let (mut conn, input) = connection(10);
        assert!(conn.ping().now_or_never().is_none());
        assert_eq!(conn.inner.stream.waiting, [Waiting::Result]);
        input.lock().unwrap().extend(packet(&[0, 0, 0, 2, 0, 0, 0]));
        conn.inner.stream.wait_until_ready().await.unwrap();
        assert!(conn.inner.stream.waiting.is_empty());
    });
}
