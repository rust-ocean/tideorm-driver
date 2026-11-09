use driver::sqlite::SqlitePoolOptions;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

fn run(future: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future);
}

#[test]
fn independent_options_defaults_clone_and_debug() {
    let defaults = SqlitePoolOptions::new();
    assert_eq!(defaults.get_connect_timeout(), Duration::from_secs(30));
    assert_eq!(defaults.get_acquire_timeout(), Duration::from_secs(30));
    assert_eq!(defaults.get_test_before_acquire_if_idle_for(), None);
    assert!(defaults.get_test_before_acquire());
    let options = defaults
        .connect_timeout(Duration::from_secs(2))
        .acquire_timeout(Duration::from_secs(7))
        .test_before_acquire_if_idle_for(Duration::from_secs(3))
        .test_before_acquire(true)
        .clone();
    assert_eq!(options.get_connect_timeout(), Duration::from_secs(2));
    assert_eq!(options.get_acquire_timeout(), Duration::from_secs(7));
    assert_eq!(
        options.get_test_before_acquire_if_idle_for(),
        Some(Duration::from_secs(3))
    );
    assert!(options.get_test_before_acquire());
    let debug = format!("{options:?}");
    assert!(debug.contains("connect_timeout: 2s"), "{debug}");
    assert!(debug.contains("acquire_timeout: 7s"), "{debug}");
    assert_eq!(
        options
            .test_before_acquire_if_idle_for(None)
            .get_test_before_acquire_if_idle_for(),
        None
    );
}

#[test]
fn connect_timeout_includes_after_connect_and_releases_capacity() {
    run(async {
        for min in [0, 1] {
            let calls = Arc::new(AtomicUsize::new(0));
            let captured = calls.clone();
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .min_connections(min)
                .idle_timeout(None)
                .max_lifetime(None)
                .connect_timeout(Duration::from_millis(60))
                .acquire_timeout(Duration::from_secs(2))
                .after_connect(move |_, _| {
                    let attempt = captured.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async move {
                        if attempt == 0 {
                            std::future::pending::<()>().await;
                        }
                        Ok(())
                    })
                })
                .connect_lazy("sqlite::memory:")
                .unwrap();
            let start = Instant::now();
            if min == 0 {
                assert!(matches!(
                    pool.acquire().await,
                    Err(driver::Error::PoolTimedOut)
                ));
            } else {
                // The lazy pool opens its minimum in maintenance, without a checkout.
                tokio::time::timeout(Duration::from_secs(1), async {
                    while calls.load(Ordering::SeqCst) == 0 || pool.size() != 0 {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap();
            }
            assert!(start.elapsed() >= Duration::from_millis(60));
            assert!(start.elapsed() < Duration::from_secs(1));
            assert_eq!(pool.size(), 0);
            let connection = pool.acquire().await.unwrap();
            assert_eq!(pool.size(), 1, "timeout must release the reserved slot");
            connection.close().await.unwrap();
            pool.close().await;
        }
    });
}

#[test]
fn each_retry_gets_a_fresh_connect_deadline() {
    run(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let captured = calls.clone();
        let pool = SqlitePoolOptions::new()
            .connect_timeout(Duration::from_millis(200))
            .acquire_timeout(Duration::from_secs(3))
            .after_connect(move |_, _| {
                let attempt = captured.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(120)).await;
                    if attempt == 0 {
                        Err(driver::Error::Protocol("retry fixture".into()))
                    } else {
                        Ok(())
                    }
                })
            })
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let start = Instant::now();
        let connection = pool.acquire().await.unwrap();
        assert!(start.elapsed() >= Duration::from_millis(240));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        connection.close().await.unwrap();
        pool.close().await;
    });
}

#[test]
fn checkout_deadline_can_interrupt_a_longer_connection_attempt() {
    run(async {
        let pool = SqlitePoolOptions::new()
            .connect_timeout(Duration::from_secs(2))
            .acquire_timeout(Duration::from_millis(60))
            .after_connect(|_, _| Box::pin(std::future::pending()))
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let start = Instant::now();
        assert!(matches!(
            pool.acquire().await,
            Err(driver::Error::PoolTimedOut)
        ));
        assert!(start.elapsed() >= Duration::from_millis(60));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(pool.size(), 0);
        pool.close().await;
    });
}
