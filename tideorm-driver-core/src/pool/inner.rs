use super::connection::{Floating, Idle, Live};
use super::size_guard::DecrementSizeGuard;
use crate::connection::ConnectOptions;
use crate::connection::Connection;
use crate::database::Database;
use crate::error::Error;
use crate::pool::close_event::deadline_as_timeout;
use crate::pool::{CloseEvent, PoolOptions};
use crossbeam_queue::ArrayQueue;

use crate::sync::{AsyncSemaphore, AsyncSemaphoreReleaser};

use std::cmp;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::logger::private_level_filter_to_trace_level;
use crate::pool::options::PoolConnectionMetadata;
use crate::private_tracing_dynamic_event;
use std::time::{Duration, Instant};
use tracing::Level;

pub(crate) struct PoolInner<DB: Database> {
    pub(super) connect_options: RwLock<Arc<<DB::Connection as Connection>::Options>>,
    pub(super) idle_conns: ArrayQueue<Idle<DB>>,
    pub(super) semaphore: AsyncSemaphore,
    pub(super) size: AtomicU32,
    pub(super) num_idle: AtomicUsize,
    is_closed: AtomicBool,
    pub(super) on_closed: event_listener::Event,
    pub(super) options: PoolOptions<DB>,
    pub(crate) acquire_time_level: Option<Level>,
    pub(crate) acquire_slow_level: Option<Level>,
}

impl<DB: Database> PoolInner<DB> {
    pub(super) fn new_arc(
        options: PoolOptions<DB>,
        connect_options: <DB::Connection as Connection>::Options,
    ) -> Arc<Self> {
        let capacity = options.max_connections as usize;

        let pool = Self {
            connect_options: RwLock::new(Arc::new(connect_options)),
            idle_conns: ArrayQueue::new(capacity),
            semaphore: AsyncSemaphore::new(capacity),
            size: AtomicU32::new(0),
            num_idle: AtomicUsize::new(0),
            is_closed: AtomicBool::new(false),
            on_closed: event_listener::Event::new(),
            acquire_time_level: private_level_filter_to_trace_level(options.acquire_time_level),
            acquire_slow_level: private_level_filter_to_trace_level(options.acquire_slow_level),
            options,
        };

        let pool = Arc::new(pool);

        spawn_maintenance_tasks(&pool);

        pool
    }

    pub(super) fn size(&self) -> u32 {
        self.size.load(Ordering::Acquire)
    }

    pub(super) fn num_idle(&self) -> usize {
        // We don't use `self.idle_conns.len()` as it waits for the internal
        // head and tail pointers to stop changing for a moment before calculating the length,
        // which may take a long time at high levels of churn.
        //
        // By maintaining our own atomic count, we avoid that issue entirely.
        self.num_idle.load(Ordering::Acquire)
    }

    pub(super) fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Acquire)
    }

    fn mark_closed(&self) {
        self.is_closed.store(true, Ordering::Release);
        self.on_closed.notify(usize::MAX);
    }

    pub(super) fn close(self: &Arc<Self>) -> impl Future<Output = ()> + '_ {
        self.mark_closed();

        async move {
            let _permits = self.semaphore.acquire(self.options.max_connections).await;

            while let Some(idle) = self.idle_conns.pop() {
                let _ = idle.live.raw.close().await;
            }

            self.num_idle.store(0, Ordering::Release);
            self.size.store(0, Ordering::Release);
        }
    }

    pub(crate) fn close_event(&self) -> CloseEvent {
        CloseEvent {
            listener: (!self.is_closed()).then(|| self.on_closed.listen()),
        }
    }

    /// Wait for a permit from `self.semaphore`, or fail once the pool closes.
    async fn acquire_permit(self: &Arc<Self>) -> Result<AsyncSemaphoreReleaser<'_>, Error> {
        self.close_event().do_until(self.semaphore.acquire(1)).await
    }

    #[inline]
    pub(super) fn try_acquire(self: &Arc<Self>) -> Option<Floating<DB, Idle<DB>>> {
        if self.is_closed() {
            return None;
        }

        let permit = self.semaphore.try_acquire(1)?;

        self.pop_idle(permit).ok()
    }

    fn pop_idle<'a>(
        self: &'a Arc<Self>,
        permit: AsyncSemaphoreReleaser<'a>,
    ) -> Result<Floating<DB, Idle<DB>>, AsyncSemaphoreReleaser<'a>> {
        if let Some(idle) = self.idle_conns.pop() {
            self.num_idle.fetch_sub(1, Ordering::AcqRel);
            Ok(Floating::from_idle(idle, (*self).clone(), permit))
        } else {
            Err(permit)
        }
    }

    pub(super) fn release(&self, floating: Floating<DB, Live<DB>>) {
        // `options.after_release` and other checks are in `PoolConnection::return_to_pool()`.

        let Floating { inner: idle, guard } = floating.into_idle();

        if self.idle_conns.push(idle).is_err() {
            panic!("BUG: connection queue overflow in release()");
        }

        // NOTE: we need to make sure we drop the permit *after* we push to the idle queue
        // don't decrease the size
        guard.release_permit();

        self.num_idle.fetch_add(1, Ordering::AcqRel);
    }

    /// Try to atomically increment the pool size for a new connection.
    ///
    /// Returns `Err` if the pool is at max capacity already or is closed.
    pub(super) fn try_increment_size<'a>(
        self: &'a Arc<Self>,
        permit: AsyncSemaphoreReleaser<'a>,
    ) -> Result<DecrementSizeGuard<DB>, AsyncSemaphoreReleaser<'a>> {
        let mut size = self.size.load(Ordering::Acquire);
        loop {
            let next = size
                .checked_add(1)
                .filter(|next| *next <= self.options.max_connections);
            // the pool is at max capacity or is closed
            let Some(next) = next.filter(|_| !self.is_closed()) else {
                return Err(permit);
            };
            match self
                .size
                .compare_exchange_weak(size, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return Ok(DecrementSizeGuard::from_permit((*self).clone(), permit)),
                Err(current) => size = current,
            }
        }
    }

    pub(super) async fn acquire(self: &Arc<Self>) -> Result<Floating<DB, Live<DB>>, Error> {
        if self.is_closed() {
            return Err(Error::PoolClosed);
        }

        let acquire_started_at = Instant::now();
        let deadline = acquire_started_at + self.options.acquire_timeout;

        let acquired = crate::rt::timeout(
            self.options.acquire_timeout,
            async {
                loop {
                    // Handles the close-event internally
                    let permit = self.acquire_permit().await?;

                    // First attempt to pop a connection from the idle queue.
                    let guard = match self.pop_idle(permit) {

                        // Then, check that we can use it...
                        Ok(conn) => match check_idle_conn(conn, &self.options).await {

                            // All good!
                            Ok(live) => return Ok(live),

                            // if the connection isn't usable for one reason or another,
                            // we get the `DecrementSizeGuard` back to open a new one
                            Err(guard) => guard,
                        },
                        Err(permit) => if let Ok(guard) = self.try_increment_size(permit) {
                            // we can open a new connection
                            guard
                        } else {
                            // This can happen if the pool was closed between `acquire_permit()`
                            // and `try_increment_size()`.
                            tracing::debug!("woke but was unable to acquire idle connection or open new one; retrying");
                            // If so, we're likely in the current-thread runtime if it's Tokio,
                            // and so we should yield to let any spawned return_to_pool() tasks
                            // execute.
                            crate::rt::yield_now().await;
                            continue;
                        }
                    };

                    // Attempt to connect...
                    return self.connect(deadline, guard).await;
                }
            }
        )
            .await
            .map_err(|_| Error::PoolTimedOut)??;

        let acquired_after = acquire_started_at.elapsed();

        let acquire_slow_level = self
            .acquire_slow_level
            .filter(|_| acquired_after > self.options.acquire_slow_threshold);

        if let Some(level) = acquire_slow_level {
            private_tracing_dynamic_event!(
                target: "tideorm::driver::pool::acquire",
                level,
                acquired_after_secs = acquired_after.as_secs_f64(),
                slow_acquire_threshold_secs = self.options.acquire_slow_threshold.as_secs_f64(),
                "acquired connection, but time to acquire exceeded slow threshold"
            );
        } else if let Some(level) = self.acquire_time_level {
            private_tracing_dynamic_event!(
                target: "tideorm::driver::pool::acquire",
                level,
                acquired_after_secs = acquired_after.as_secs_f64(),
                "acquired connection"
            );
        }

        Ok(acquired)
    }

    pub(super) async fn connect(
        self: &Arc<Self>,
        deadline: Instant,
        guard: DecrementSizeGuard<DB>,
    ) -> Result<Floating<DB, Live<DB>>, Error> {
        if self.is_closed() {
            return Err(Error::PoolClosed);
        }

        let mut backoff = Duration::from_millis(10);
        let max_backoff = deadline_as_timeout(deadline)? / 5;

        loop {
            let timeout = deadline_as_timeout(deadline)?.min(self.options.connect_timeout);

            // clone the connect options arc so it can be used without holding the RwLockReadGuard
            // across an async await point
            let connect_options = self
                .connect_options
                .read()
                .expect("write-lock holder panicked")
                .clone();

            let attempt = async {
                let mut raw = connect_options.connect().await?;
                if let Some(callback) = &self.options.after_connect {
                    let meta = PoolConnectionMetadata {
                        age: Duration::ZERO,
                        idle_for: Duration::ZERO,
                    };
                    if let Err(error) = callback(&mut raw, meta).await {
                        tracing::error!(%error, "error returned from after_connect");
                        let _ = raw.close_hard().await;
                        return Ok(None);
                    }
                }
                Ok(Some(raw))
            };

            match crate::rt::timeout(timeout, attempt).await {
                Ok(Ok(Some(raw))) => return Ok(Floating::new_live(raw, guard)),
                Ok(Ok(None)) => (),

                // an IO error while connecting is assumed to be the system starting up
                Ok(Err(Error::Io(e))) if e.kind() == std::io::ErrorKind::ConnectionRefused => (),

                // We got a transient database error, retry.
                Ok(Err(Error::Database(error))) if error.is_transient_in_connect_phase() => (),

                // Any other error while connection should immediately
                // terminate and bubble the error up
                Ok(Err(e)) => return Err(e),

                // timed out
                Err(_) => return Err(Error::PoolTimedOut),
            }

            // If the connection is refused, wait in exponentially
            // increasing steps for the server to come up,
            // capped by a factor of the remaining time until the deadline
            crate::rt::sleep(backoff.min(deadline_as_timeout(deadline)?)).await;
            backoff = cmp::min(backoff * 2, max_backoff);
        }
    }

    /// Try to maintain `min_connections`, returning any errors (including `PoolTimedOut`).
    pub async fn try_min_connections(self: &Arc<Self>, deadline: Instant) -> Result<(), Error> {
        while self.size() < self.options.min_connections {
            // Don't wait for a semaphore permit.
            //
            // If no extra permits are available then we shouldn't be trying to spin up
            // connections anyway.
            let Some(permit) = self.semaphore.try_acquire(1) else {
                return Ok(());
            };

            // We must always obey `max_connections`.
            let Some(guard) = self.try_increment_size(permit).ok() else {
                return Ok(());
            };

            // We skip `after_release` since the connection was never provided to user code
            // besides `after_connect`, if they set it.
            self.release(self.connect(deadline, guard).await?);
        }

        Ok(())
    }

    /// Attempt to maintain `min_connections`, logging if unable.
    pub async fn min_connections_maintenance(self: &Arc<Self>, deadline: Option<Instant>) {
        let deadline = deadline.unwrap_or_else(|| {
            // Arbitrary default deadline if the caller doesn't care.
            Instant::now() + Duration::from_secs(300)
        });

        match self.try_min_connections(deadline).await {
            Ok(()) => (),
            Err(Error::PoolClosed) => (),
            Err(Error::PoolTimedOut) => {
                tracing::debug!("unable to complete `min_connections` maintenance before deadline")
            }
            Err(error) => tracing::debug!(%error, "error while maintaining min_connections"),
        }
    }
}

impl<DB: Database> Drop for PoolInner<DB> {
    fn drop(&mut self) {
        self.mark_closed();
    }
}

/// Returns `true` if the connection has exceeded `options.max_lifetime` if set, `false` otherwise.
pub(super) fn is_beyond_max_lifetime<DB: Database>(
    live: &Live<DB>,
    options: &PoolOptions<DB>,
) -> bool {
    options
        .max_lifetime
        .is_some_and(|max| live.created_at.elapsed() > max)
}

/// Returns `true` if the connection has exceeded `options.idle_timeout` if set, `false` otherwise.
fn is_beyond_idle_timeout<DB: Database>(idle: &Idle<DB>, options: &PoolOptions<DB>) -> bool {
    options
        .idle_timeout
        .is_some_and(|timeout| idle.idle_since.elapsed() > timeout)
}

async fn check_idle_conn<DB: Database>(
    mut conn: Floating<DB, Idle<DB>>,
    options: &PoolOptions<DB>,
) -> Result<Floating<DB, Live<DB>>, DecrementSizeGuard<DB>> {
    let should_ping = options
        .test_before_acquire_if_idle_for
        .map_or(options.test_before_acquire, |threshold| {
            conn.idle_since.elapsed() >= threshold
        });
    if should_ping {
        // Check that the connection is still live
        if let Err(error) = conn.ping().await {
            // an error here means the other end has hung up or we lost connectivity
            // either way we're fine to just discard the connection
            // the error itself here isn't necessarily unexpected so WARN is too strong
            tracing::info!(%error, "ping on idle connection returned error");
            // connection is broken so don't try to close nicely
            return Err(conn.close_hard().await);
        }
    }

    if let Some(test) = &options.before_acquire {
        let meta = conn.metadata();
        match test(&mut conn.live.raw, meta).await {
            Ok(false) => {
                // connection was rejected by user-defined hook, close nicely
                return Err(conn.close().await);
            }

            Err(error) => {
                tracing::warn!(%error, "error from `before_acquire`");
                // connection is broken so don't try to close nicely
                return Err(conn.close_hard().await);
            }

            Ok(true) => {}
        }
    }

    // No need to re-connect; connection is alive or we don't care
    Ok(conn.into_live())
}

fn spawn_maintenance_tasks<DB: Database>(pool: &Arc<PoolInner<DB>>) {
    // NOTE: use `pool_weak` for the maintenance tasks
    // so they don't keep `PoolInner` from being dropped.
    let pool_weak = Arc::downgrade(pool);

    let period = match (pool.options.max_lifetime, pool.options.idle_timeout) {
        (Some(it), None) | (None, Some(it)) => it,

        (Some(a), Some(b)) => cmp::min(a, b),

        (None, None) => {
            if pool.options.min_connections > 0 {
                crate::rt::spawn(async move {
                    if let Some(pool) = pool_weak.upgrade() {
                        pool.min_connections_maintenance(None).await;
                    }
                });
            }

            return;
        }
    };

    // Immediately cancel this task if the pool is closed.
    let mut close_event = pool.close_event();

    crate::rt::spawn(async move {
        let _ = close_event
            .do_until(async {
                // If the last handle to the pool was dropped while we were sleeping
                while let Some(pool) = pool_weak.upgrade() {
                    if pool.is_closed() {
                        return;
                    }

                    let next_run = Instant::now() + period;

                    // Go over all idle connections, check for idleness and lifetime,
                    // and if we have fewer than min_connections after reaping a connection,
                    // open a new one immediately. Note that other connections may be popped from
                    // the queue in the meantime - that's fine, there is no harm in checking more
                    for _ in 0..pool.num_idle() {
                        if let Some(conn) = pool.try_acquire() {
                            if is_beyond_idle_timeout(&conn, &pool.options)
                                || is_beyond_max_lifetime(&conn, &pool.options)
                            {
                                let _ = conn.close().await;
                                pool.min_connections_maintenance(Some(next_run)).await;
                            } else {
                                pool.release(conn.into_live());
                            }
                        }
                    }

                    // Don't hold a reference to the pool while sleeping.
                    drop(pool);

                    if let Some(duration) = next_run.checked_duration_since(Instant::now()) {
                        // `async-std` doesn't have a `sleep_until()`
                        crate::rt::sleep(duration).await;
                    } else {
                        // `next_run` is in the past, just yield.
                        crate::rt::yield_now().await;
                    }
                }
            })
            .await;
    });
}
