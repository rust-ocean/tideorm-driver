use super::*;
use driver_core::executor::Executor;
use driver_core::pool::PoolOptions;
use driver_core::row::Row;
use std::time::{Duration, Instant};

async fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        driver_core::rt::sleep(Duration::from_millis(5)).await;
    }
}

#[test]
fn a_checkout_takes_the_connection_released_last() {
    driver_core::rt::test_block_on(async {
        let pool = PoolOptions::<Sqlite>::new()
            .max_connections(3)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let mut held = Vec::new();
        // a temporary table is the connection's own
        for tag_it in [
            "CREATE TEMP TABLE me AS SELECT 0 AS tag",
            "CREATE TEMP TABLE me AS SELECT 1 AS tag",
            "CREATE TEMP TABLE me AS SELECT 2 AS tag",
        ] {
            let mut conn = pool.acquire().await.unwrap();
            conn.execute(tag_it).await.unwrap();
            held.push(conn);
        }
        for (released, conn) in held.into_iter().enumerate() {
            drop(conn);
            wait_until("the connection is back", || pool.num_idle() == released + 1).await;
        }

        let mut conn = pool.acquire().await.unwrap();
        let tag: i64 = driver_core::query::query("SELECT tag FROM me")
            .fetch_one(&mut *conn)
            .await
            .unwrap()
            .get(0);
        assert_eq!(tag, 2);
        drop(conn);
        pool.close().await;
    });
}

#[test]
fn an_idle_connection_reaches_its_idle_timeout() {
    driver_core::rt::test_block_on(async {
        let pool = PoolOptions::<Sqlite>::new()
            .max_connections(2)
            .min_connections(0)
            .idle_timeout(Duration::from_millis(200))
            .max_lifetime(None)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let first = pool.acquire().await.unwrap();
        let second = pool.acquire().await.unwrap();
        drop((first, second));
        wait_until("both are back", || pool.num_idle() == 2).await;
        // the reaper's own checks must not restart the idle time
        wait_until("the reaper closed both", || pool.size() == 0).await;
        pool.close().await;
    });
}
