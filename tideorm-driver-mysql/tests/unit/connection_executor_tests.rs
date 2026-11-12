use super::*;
use crate::common::StatementCache;
use crate::connection::MySqlConnectionInner;
use crate::test_support::{fuzz, packet};
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

/// Runs a statement whose reply is `reply` and reads every result and value it gives, until
/// the reply runs out or is refused: in the binary format, after its prepare, when `binary`.
fn read_reply(reply: &[u8], binary: bool) {
    use driver_core::row::Row as _;

    let (mut conn, input) = connection(10);
    let mut script = if binary {
        [prepare_ok(1), definition()].concat()
    } else {
        Vec::new()
    };
    script.extend(reply);
    *input.lock().unwrap() = script;
    let arguments = binary.then(one_value);
    let Some(Ok(stream)) = conn
        .run("SELECT ?".into_sql_str(), arguments, true)
        .now_or_never()
    else {
        return;
    };
    let mut stream = pin!(stream);
    // a reply can announce more results without end
    for _ in 0..64 {
        match stream.try_next().now_or_never() {
            Some(Ok(Some(Either::Right(row)))) => {
                for i in 0..row.columns.len() {
                    let _ = row.try_get_raw(i).map(|value| value.decode_lossless());
                }
            }
            Some(Ok(Some(Either::Left(_)))) => {}
            _ => break,
        }
    }
}

/// Replies of each kind: OK, ERR, and a result set of two rows, the second followed by
/// another result.
fn replies(binary: bool) -> Vec<Vec<u8>> {
    let rows = if binary {
        // the header, the NULL bitmap (offset by 2 bits), then the values
        [
            packet(&[0, 0, 3, b'a', b'b', b'c', 1, b'x']),
            packet(&[0, 0b1000, 2, b'y', b'z']),
        ]
    } else {
        [
            packet(&[3, b'a', b'b', b'c', 1, b'x']),
            packet(&[2, b'y', b'z', 0xfb]),
        ]
    };
    let result_set = [packet(&[2]), definition(), definition(), rows.concat()].concat();
    vec![
        packet(&[0, 1, 5, 2, 0, 0, 0]),
        packet(b"\xff\x28\x04#42000You have an error"),
        [&result_set[..], &packet(&[0xfe, 0, 0, 2, 0, 0, 0])].concat(),
        [
            &result_set[..],
            &packet(&[0xfe, 0, 0, 0x0a, 0, 0, 0]), // more results
            &packet(&[0, 0, 0, 2, 0, 0, 0]),
        ]
        .concat(),
    ]
}

/// A reply, in either format, is read or refused without a panic or an allocation its bytes do
/// not justify.
#[test]
fn any_reply_is_read_or_refused_without_panicking() {
    for (seed, binary) in [(20, false), (21, true)] {
        fuzz(seed, 3000, &replies(binary), |reply| {
            read_reply(reply, binary)
        });
    }
}

/// A result set's column count is the server's, up to 2^64 - 1, and reserves no memory for
/// columns whose definitions have not arrived.
#[test]
fn a_forged_column_count_reserves_nothing_absurd() {
    let counts = [u64::MAX, 1 << 32, 1 << 24];
    let replies: Vec<Vec<u8>> = counts
        .iter()
        .map(|count| packet(&[&[0xfe][..], &count.to_le_bytes()].concat()))
        .collect();
    for binary in [false, true] {
        fuzz(22, 0, &replies, |reply| read_reply(reply, binary));
    }
}

/// `fetch_optional` reads the whole reply before it returns its first row, so an error the
/// server sends after that row fails this statement and the connection is left ready.
#[test]
fn fetch_optional_reports_an_error_sent_after_its_first_row() {
    driver_core::rt::test_block_on(async {
        let head = [packet(&[1]), definition(), packet(&[1, b'x'])].concat();
        for (tail, fails) in [
            (
                packet(b"\xff\x28\x04#70100Query execution was interrupted"),
                true,
            ),
            (
                [packet(&[1, b'y']), packet(&[0xfe, 0, 0, 2, 0, 0, 0])].concat(),
                false,
            ),
        ] {
            let (mut conn, input) = connection(10);
            *input.lock().unwrap() = [&head[..], &tail[..]].concat();
            let result = (&mut conn).fetch_optional("SELECT a FROM t").await;
            assert_eq!(result.is_err(), fails, "{:?}", result.as_ref().err());
            assert!(conn.inner.stream.waiting.is_empty());
        }
    });
}
