use super::*;
use crate::test_support::packet;
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
