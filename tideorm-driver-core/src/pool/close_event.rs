//! The future a pool resolves when it closes, and the deadline helper its waits use.

use std::future::Future;
use std::pin::{pin, Pin};
use std::task::{ready, Context, Poll};
use std::time::{Duration, Instant};

use event_listener::EventListener;
use futures_core::FusedFuture;
use futures_util::FutureExt;

use crate::error::Error;

/// A future that resolves when the pool is closed.
///
/// See [`Pool::close_event()`](super::Pool::close_event) for details.
pub struct CloseEvent {
    pub(super) listener: Option<EventListener>,
}

impl CloseEvent {
    /// Execute the given future until it returns or the pool is closed.
    ///
    /// Cancels the future and returns `Err(PoolClosed)` if/when the pool is closed.
    /// If the pool was already closed, the future is never run.
    pub async fn do_until<Fut: Future>(&mut self, fut: Fut) -> Result<Fut::Output, Error> {
        // Check that the pool wasn't closed already.
        //
        // We use `poll_immediate()` as it will use the correct waker instead of
        // a no-op one like `.now_or_never()`, but it won't actually suspend execution here.
        futures_util::future::poll_immediate(&mut *self)
            .await
            .map_or(Ok(()), |_| Err(Error::PoolClosed))?;

        let mut fut = pin!(fut);

        // I find that this is clearer in intent than `futures_util::future::select()`
        // or `futures_util::select_biased!{}` (which isn't enabled anyway).
        std::future::poll_fn(|cx| {
            // Poll `fut` first as the wakeup event is more likely for it than `self`.
            if let Poll::Ready(ret) = fut.as_mut().poll(cx) {
                return Poll::Ready(Ok(ret));
            }

            // Can't really factor out mapping to `Err(Error::PoolClosed)` though it seems like
            // we should because that results in a different `Ok` type each time.
            //
            // Ideally we'd map to something like `Result<!, Error>` but using `!` as a type
            // is not allowed on stable Rust yet.
            self.poll_unpin(cx).map(|_| Err(Error::PoolClosed))
        })
        .await
    }
}

impl Future for CloseEvent {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(listener) = &mut self.listener {
            ready!(listener.poll_unpin(cx));
        }

        // `EventListener` doesn't like being polled after it yields, and even if it did it
        // would probably just wait for the next event, neither of which we want.
        //
        // So this way, once we get our close event, we fuse this future to immediately return.
        self.listener = None;

        Poll::Ready(())
    }
}

impl FusedFuture for CloseEvent {
    fn is_terminated(&self) -> bool {
        self.listener.is_none()
    }
}

/// get the time between the deadline and now and use that as our timeout
///
/// returns `Error::PoolTimedOut` if the deadline is in the past
pub(super) fn deadline_as_timeout(deadline: Instant) -> Result<Duration, Error> {
    deadline
        .checked_duration_since(Instant::now())
        .ok_or(Error::PoolTimedOut)
}
