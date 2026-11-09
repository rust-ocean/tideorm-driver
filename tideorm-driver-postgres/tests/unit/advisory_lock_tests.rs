use super::*;
use crate::test_support::{Answer, FakeServer};
use driver_core::connection::Connection;
use driver_core::rt::{test_block_on, timeout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[test]
fn a_guard_unlocks_once_whether_dropped_or_released() {
    let server = FakeServer::start(|_| Answer::Done);

    test_block_on(async {
        let lock = PgAdvisoryLock::with_key(PgAdvisoryLockKey::IntPair(1, 2));
        let mut conn = PgConnection::connect(&server.url).await.unwrap();

        // A dropped guard queues the unlock for the connection's next use.
        drop(PgAdvisoryLockGuard::new(lock.clone(), &mut conn));
        conn.execute("SELECT 1").await.unwrap();

        let conn = PgAdvisoryLockGuard::new(lock, conn)
            .release_now()
            .await
            .unwrap();
        conn.close().await.unwrap();
    });

    assert_eq!(
        server.queries(),
        [
            "1:SELECT pg_advisory_unlock(1, 2)",
            "1:SELECT 1",
            "1:SELECT pg_advisory_unlock(1, 2)",
        ]
    );
}

#[test]
fn a_cancelled_release_now_still_unlocks_once() {
    let reply = Arc::new(AtomicBool::new(false));
    let gate = reply.clone();
    let server = FakeServer::start(move |sql| {
        if sql.contains("unlock") {
            Answer::DoneAfter(gate.clone())
        } else {
            Answer::Done
        }
    });

    test_block_on(async {
        let lock = PgAdvisoryLock::with_key(PgAdvisoryLockKey::BigInt(42));
        let mut conn = PgConnection::connect(&server.url).await.unwrap();

        let guard = PgAdvisoryLockGuard::new(lock, &mut conn);
        let released = timeout(Duration::from_millis(50), guard.release_now()).await;
        assert!(released.is_err(), "the server holds the reply back");

        reply.store(true, Ordering::SeqCst);
        conn.execute("SELECT 1").await.unwrap();
        conn.close().await.unwrap();
    });

    assert_eq!(
        server.queries(),
        ["1:SELECT pg_advisory_unlock(42)", "1:SELECT 1"]
    );
}
