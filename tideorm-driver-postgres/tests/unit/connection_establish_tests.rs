//! How a pool opens connections and hands them out: handshakes cut short, many waiters, closing.

use crate::test_support::{read_body, send, serve, Answer};
use crate::{PgPool, PgPoolOptions};
use driver_core::executor::Executor;
use driver_core::rt::{test_block_on, timeout, yield_now};
use futures_util::future::join_all;
use std::io::Read;
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn a_handshake_cut_short_gives_its_pool_slot_back() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "postgres://test@{}/test?sslmode=disable",
        listener.local_addr().unwrap()
    );
    thread::spawn(move || {
        let mut stalled = Vec::new();
        for (session, stream) in listener.incoming().enumerate() {
            let mut stream = stream.unwrap();
            match session {
                // Never answer the start-up message.
                0..=2 => stalled.push(stream),
                // Hang up after reading part of it.
                3 => drop(stream.read(&mut [0; 4])),
                _ => drop(serve(
                    Ok(stream),
                    session,
                    &|_| Answer::Done,
                    &Mutex::default(),
                )),
            }
        }
    });

    test_block_on(async {
        let pool: PgPool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_millis(200))
            .connect_lazy(&url)
            .unwrap();
        for _ in 0..3 {
            let error = pool.acquire().await.unwrap_err();
            assert!(matches!(error, crate::Error::PoolTimedOut), "{error:?}");
            assert_eq!(pool.size(), 0);
        }
        // With the one slot leaked, this would time out too.
        let error = pool.acquire().await.unwrap_err();
        assert!(!matches!(error, crate::Error::PoolTimedOut), "{error:?}");
        assert_eq!(pool.size(), 0);

        let mut connection = pool.acquire().await.unwrap();
        connection.execute("SELECT 1").await.unwrap();
        assert_eq!(pool.size(), 1);
        drop(connection);
        pool.close().await;
    });
}

/// A PostgreSQL server serving every session on a thread of its own, refusing every
/// `refuse_every`th start-up as a wrong password, and counting the most sessions open at once.
fn concurrent_server(refuse_every: usize) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "postgres://test@{}/test?sslmode=disable",
        listener.local_addr().unwrap()
    );
    let peak = Arc::new(AtomicUsize::new(0));
    let most = peak.clone();
    let open = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        for (session, stream) in listener.incoming().enumerate() {
            let (open, most) = (open.clone(), most.clone());
            thread::spawn(move || {
                most.fetch_max(open.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                let mut stream = stream.unwrap();
                if session % refuse_every == refuse_every - 1 {
                    let _ = read_body(&mut stream);
                    let _ = send(&mut stream, b'E', b"SFATAL\0C28P01\0Mrefused\0\0");
                } else {
                    let _ = serve(Ok(stream), session, &|_| Answer::Done, &Mutex::default());
                }
                open.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    (url, peak)
}

fn pool(url: &str, max: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(max)
        .acquire_timeout(Duration::from_secs(10))
        .connect_lazy(url)
        .unwrap()
}

#[test]
fn waiters_are_served_in_turn_and_never_past_the_maximum() {
    let (url, peak) = concurrent_server(4);
    test_block_on(async {
        let pool = pool(&url, 3);
        let holding = AtomicUsize::new(0);
        let order = Mutex::new(Vec::new());
        let results = join_all((0..300).map(|waiter| {
            let (pool, holding, order) = (&pool, &holding, &order);
            async move {
                let mut connection = pool.acquire().await?;
                order.lock().unwrap().push(waiter);
                assert!(holding.fetch_add(1, Ordering::SeqCst) < 3);
                connection.execute("SELECT 1").await?;
                yield_now().await;
                holding.fetch_sub(1, Ordering::SeqCst);
                Ok::<_, crate::Error>(())
            }
        }))
        .await;

        // A refused start-up fails its own waiter only.
        for result in &results {
            match result {
                Ok(()) | Err(crate::Error::Database(_)) => {}
                Err(other) => panic!("{other:?}"),
            }
        }
        assert!(results.iter().filter(|result| result.is_ok()).count() > 250);
        // Permits go out in turn; a waiter that must open a connection can still finish after
        // one of the next three, which took an idle one.
        let order = order.into_inner().unwrap();
        for (at, waiter) in order.iter().enumerate() {
            let overtaken_by = order[..at].iter().filter(|before| *before > waiter).count();
            assert!(
                overtaken_by <= 3,
                "{waiter} overtaken by {overtaken_by}: {order:?}"
            );
        }
        assert!(pool.size() <= 3);
        pool.close().await;
    });
    assert!(
        peak.load(Ordering::SeqCst) <= 3,
        "{peak:?} sessions at once"
    );
}

#[test]
fn cancelled_waiters_leave_no_permit_behind() {
    let (url, _) = concurrent_server(usize::MAX);
    test_block_on(async {
        let pool = pool(&url, 3);
        join_all((0..200_u64).map(|waiter| {
            let pool = &pool;
            async move {
                // Every other waiter gives up after 0 to 4 ms, at any point of its acquire.
                let patience = Duration::from_millis(waiter % 5);
                let work = async {
                    let mut connection = pool.acquire().await.unwrap();
                    connection.execute("SELECT 1").await.unwrap();
                };
                if waiter % 2 == 0 {
                    let _ = timeout(patience, work).await;
                } else {
                    work.await;
                }
            }
        }))
        .await;

        // All three permits are back: three connections can be held at once.
        let started = Instant::now();
        let held = join_all((0..3).map(|_| pool.acquire())).await;
        assert!(held.iter().all(Result::is_ok), "{held:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(pool.size() <= 3);
        drop(held);
        pool.close().await;
    });
}

#[test]
fn closing_the_pool_turns_its_waiters_away() {
    let (url, _) = concurrent_server(usize::MAX);
    test_block_on(async {
        let pool = pool(&url, 1);
        let held = pool.acquire().await.unwrap();
        let started = Instant::now();
        let waiters = join_all((0..50).map(|_| pool.acquire()));
        let closing = async {
            yield_now().await;
            let close = pool.close();
            drop(held);
            close.await;
        };
        let (results, ()) = futures_util::future::join(waiters, closing).await;
        for result in results {
            assert!(
                matches!(result, Err(crate::Error::PoolClosed)),
                "{result:?}"
            );
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(pool.size(), 0);
    });
}
