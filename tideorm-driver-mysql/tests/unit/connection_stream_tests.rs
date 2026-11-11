use super::*;
use crate::test_support::{fuzz, packet};
use driver_core::net::ScriptedSocket;

#[test]
fn begin_and_rollback_replies_update_state() {
    driver_core::rt::test_block_on(async {
        let mut input = packet(&[0, 0, 0, 3, 0, 0, 0]);
        input.extend(packet(&[0, 0, 0, 2, 0, 0, 0]));
        let mut stream =
            MySqlStream::with_socket(&MySqlConnectOptions::new(), ScriptedSocket::new(input));
        stream.waiting.push_back(Waiting::Result);
        stream.wait_until_ready().await.unwrap();
        assert!(stream.status_flags.contains(Status::SERVER_STATUS_IN_TRANS));
        stream.waiting.push_back(Waiting::Result);
        stream.wait_until_ready().await.unwrap();
        assert!(!stream.status_flags.contains(Status::SERVER_STATUS_IN_TRANS));
    });
}

#[test]
fn eof_and_ping_status_are_observed() {
    driver_core::rt::test_block_on(async {
        for deprecate in [false, true] {
            let eof = if deprecate {
                vec![0xfe, 0, 0, 1, 0, 0, 0, 2, 111, 107]
            } else {
                vec![0xfe, 0, 0, 1, 0]
            };
            let mut input = packet(&eof);
            input.extend(packet(&[0, 0, 0, 2, 0, 0, 0]));
            let mut stream =
                MySqlStream::with_socket(&MySqlConnectOptions::new(), ScriptedSocket::new(input));
            stream
                .capabilities
                .set(Capabilities::DEPRECATE_EOF, deprecate);
            stream.waiting.push_back(Waiting::Row);
            stream.wait_until_ready().await.unwrap();
            assert!(stream.status_flags.contains(Status::SERVER_STATUS_IN_TRANS));
            // A ping's OK, read as the reply it awaits.
            stream.waiting.push_back(Waiting::Result);
            stream.wait_until_ready().await.unwrap();
            assert!(!stream.status_flags.contains(Status::SERVER_STATUS_IN_TRANS));
        }
    });
}

#[test]
fn cancelled_partial_begin_reply_keeps_its_header() {
    use futures_util::FutureExt;
    driver_core::rt::test_block_on(async {
        let bytes = packet(&[0, 0, 0, 3, 0, 0, 0]);
        let socket = ScriptedSocket::new(bytes[..4].to_vec());
        let input = socket.input.clone();
        let mut stream = MySqlStream::with_socket(&MySqlConnectOptions::new(), socket);
        stream.waiting.push_back(Waiting::Result);
        assert!(stream.wait_until_ready().now_or_never().is_none());
        input.lock().unwrap().extend(&bytes[4..]);
        stream.wait_until_ready().await.unwrap();
        assert!(stream.status_flags.contains(Status::SERVER_STATUS_IN_TRANS));
        assert!(stream.waiting.is_empty());
    });
}

#[test]
fn a_read_cancelled_between_parts_leaves_the_stream_unusable() {
    use futures_util::FutureExt;
    driver_core::rt::test_block_on(async {
        // the first part of a packet of 16 MiB or more, and nothing after it
        let mut input = vec![0xff, 0xff, 0xff, 0];
        input.resize(4 + 0xFF_FF_FF, 0);
        let mut stream =
            MySqlStream::with_socket(&MySqlConnectOptions::new(), ScriptedSocket::new(input));
        stream.waiting.push_back(Waiting::Result);
        assert!(stream.recv_packet().now_or_never().is_none());
        let error = stream.wait_until_ready().await.unwrap_err();
        assert!(error.to_string().contains("between the parts"), "{error}");
    });
}

#[test]
fn a_text_row_led_by_0xfe_does_not_end_the_result_set() {
    driver_core::rt::test_block_on(async {
        // a row whose first value has 0xFF_FF_FF bytes, its length led by 0xfe
        let mut row = vec![0xfe];
        row.extend(0xFF_FF_FF_u64.to_le_bytes());
        row.resize(row.len() + 0xFF_FF_FF, b'x');
        let (first, rest) = row.split_at(0xFF_FF_FF);
        let mut input = vec![0xff, 0xff, 0xff, 0];
        input.extend(first);
        input.extend(&u32::try_from(rest.len()).unwrap().to_le_bytes()[..3]);
        input.push(1);
        input.extend(rest);
        input.extend(packet(&[0xfe, 0, 0, 1, 0, 0, 0]));
        let mut stream =
            MySqlStream::with_socket(&MySqlConnectOptions::new(), ScriptedSocket::new(input));
        stream.waiting.push_back(Waiting::Row);
        stream.wait_until_ready().await.unwrap();
        assert!(stream.status_flags.contains(Status::SERVER_STATUS_IN_TRANS));
        assert!(stream.waiting.is_empty());
    });
}

/// Skips the rest of `reply` as `wait_until_ready` does, awaiting `waiting`.
fn skip_reply(reply: &[u8], waiting: Waiting, deprecate_eof: bool) {
    use futures_util::FutureExt;
    let mut stream = MySqlStream::with_socket(
        &MySqlConnectOptions::new(),
        ScriptedSocket::new(reply.to_vec()),
    );
    stream
        .capabilities
        .set(Capabilities::DEPRECATE_EOF, deprecate_eof);
    stream.waiting.push_back(waiting);
    let _ = stream.wait_until_ready().now_or_never();
}

/// Any reply, its packets misframed included, is skipped or refused without a panic or an
/// allocation its bytes do not justify.
#[test]
fn any_reply_is_skipped_or_refused_without_panicking() {
    // six names, MariaDB's extended type info, then the fixed fields
    let definition = packet(b"\x03def\0\0\0\x01c\0\0\x0c\x2d\0\xff\0\0\0\xfd\0\0\0\0\0");
    let ok = packet(&[0, 0, 0, 2, 0, 0, 0]);
    for (seed, deprecate_eof) in [(30, true), (31, false)] {
        // an EOF packet, or the OK packet that stands for one
        let eof = |status: u8| {
            if deprecate_eof {
                packet(&[0xfe, 0, 0, status, 0, 0, 0])
            } else {
                packet(&[0xfe, 0, 0, status, 0])
            }
        };
        let metadata_end = if deprecate_eof { Vec::new() } else { eof(2) };
        let result_set = [
            packet(&[1]),
            definition.clone(),
            metadata_end,
            packet(&[1, b'x']),
        ]
        .concat();
        let replies = vec![
            ok.clone(),
            packet(b"\xff\x28\x04#42000You have an error"),
            [result_set.clone(), eof(2)].concat(),
            [result_set, eof(0x0a), ok.clone()].concat(), // more results
        ];
        fuzz(seed, 3000, &replies, |reply| {
            skip_reply(reply, Waiting::Result, deprecate_eof);
            skip_reply(reply, Waiting::Row, deprecate_eof);
        });
    }
}

/// A packet of 16 MiB or more is reassembled from its parts, whatever the parts after the first
/// claim.
#[test]
fn any_part_after_the_first_is_read_or_refused_without_panicking() {
    let mut first = vec![0xff, 0xff, 0xff, 0];
    first.resize(4 + 0xFF_FF_FF, 0);
    let tails: [&[u8]; 4] = [
        &[0, 0, 0, 1],                // an empty last part
        &[3, 0, 0, 1, 0, 0, 0],       // a last part of three bytes
        &[0xff, 0xff, 0xff, 1, 0xfe], // a part that claims 16 MiB and stops short
        &[0xff, 0xff, 0xff, 1],       // the same, with nothing after its header
    ];
    let inputs: Vec<Vec<u8>> = tails
        .iter()
        .map(|tail| [&first[..], tail].concat())
        .collect();
    fuzz(32, 0, &inputs, |input| {
        skip_reply(input, Waiting::Result, true)
    });
}
