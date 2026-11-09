use super::*;
use crate::test_support::{Answer, FakeServer};
use driver_core::rt::{test_block_on, timeout};
use std::time::Duration;

#[test]
fn listen_all_query_quotes_every_channel() {
    let cases: &[(&[&str], &str)] = &[
        (&[], ""),
        (&["test"], r#"LISTEN "test";"#),
        (
            &["channel.0", "channel.1"],
            r#"LISTEN "channel.0";LISTEN "channel.1";"#,
        ),
        // a quote inside a name is doubled, and a NUL ends the name
        (&[r#"say "hi""#], r#"LISTEN "say ""hi""";"#),
        (&["cut\0here"], r#"LISTEN "cut";"#),
    ];

    for &(channels, expected) in cases {
        assert_eq!(build_listen_all_query(channels), expected, "{channels:?}");
    }
}

#[test]
fn lost_connection_kinds() {
    let cases = [
        (io::ErrorKind::ConnectionAborted, true),
        (io::ErrorKind::ConnectionReset, true),
        (io::ErrorKind::UnexpectedEof, true),
        (io::ErrorKind::TimedOut, true),
        (io::ErrorKind::BrokenPipe, true),
        (io::ErrorKind::PermissionDenied, false),
        (io::ErrorKind::InvalidData, false),
    ];

    for (kind, lost) in cases {
        assert_eq!(is_lost_connection(kind), lost, "{kind:?}");
    }
}

#[test]
fn a_channel_is_subscribed_once_and_unlisten_forgets_it() {
    let server = FakeServer::start(|sql| {
        if sql == r#"UNLISTEN "topic""# {
            Answer::DoneThenClose
        } else {
            Answer::Done
        }
    });

    test_block_on(async {
        let mut listener = PgListener::connect(&server.url).await.unwrap();
        listener.listen("topic").await.unwrap();
        listener.listen("topic").await.unwrap();
        listener
            .listen_all(["topic", "other", "other"])
            .await
            .unwrap();
        assert_eq!(listener.channels, ["topic", "other"]);

        listener.unlisten("other").await.unwrap();
        assert_eq!(listener.channels, ["topic"]);
        listener.unlisten("topic").await.unwrap();
        assert!(listener.channels.is_empty());

        // The server ended that session: the listener reconnects and subscribes to nothing.
        let received = timeout(Duration::from_secs(10), listener.try_recv()).await;
        assert!(received.unwrap().unwrap().is_none());
    });

    let queries = server.queries();
    assert_eq!(
        queries[..5],
        [
            r#"1:LISTEN "topic""#,
            r#"1:LISTEN "topic""#,
            r#"1:LISTEN "topic";LISTEN "other";LISTEN "other";"#,
            r#"1:UNLISTEN "other""#,
            r#"1:UNLISTEN "topic""#,
        ]
    );
    assert!(
        queries[5..].iter().all(|query| query == "2:"),
        "{queries:?}"
    );
}

#[test]
fn a_refused_listen_all_subscribes_nothing() {
    let server = FakeServer::start(|sql| {
        if sql.contains(r#"LISTEN """#) {
            Answer::Fail
        } else {
            Answer::Done
        }
    });

    test_block_on(async {
        let mut listener = PgListener::connect(&server.url).await.unwrap();
        listener.listen_all(["good", ""]).await.unwrap_err();
        assert!(listener.channels.is_empty());

        listener.listen_all(["good"]).await.unwrap();
        assert_eq!(listener.channels, ["good"]);
    });
}
