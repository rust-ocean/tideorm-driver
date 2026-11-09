//! Streams written as async blocks, with no proc macro and no unsafe code.
//!
//! [`try_stream!`](crate::try_stream) runs its block as one boxed future. Each `r#yield!(value)`
//! puts the value in a one-value slot the block shares with the stream, then suspends the
//! block once. The stream sees the future pending with a value waiting and returns that value.
//! The suspension wakes nobody, so the block resumes only when the consumer asks for the next
//! item; a [`Yielder`] awaited outside its own block would stall the task.

use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};

use futures_core::future::BoxFuture;
use futures_core::stream::Stream;
use futures_core::FusedFuture;
use futures_util::future::Fuse;
use futures_util::FutureExt;

use crate::error::Error;

/// A stream of the values an async block yields, ending with the block: after its last value
/// on `Ok(())`, or with its error as the last item.
pub struct TryAsyncStream<'a, T> {
    yielder: Yielder<T>,
    body: Fuse<BoxFuture<'a, Result<(), Error>>>,
}

impl<'a, T> TryAsyncStream<'a, T> {
    /// Makes the stream of `body`, which is given the [`Yielder`] its values go through. The
    /// future it returns runs only as the stream is polled.
    pub fn new<F, Fut>(body: F) -> Self
    where
        F: FnOnce(Yielder<T>) -> Fut + Send,
        Fut: 'a + Future<Output = Result<(), Error>> + Send,
        T: 'a + Send,
    {
        let yielder = Yielder {
            slot: Arc::default(),
        };
        let body = body(Yielder {
            slot: Arc::clone(&yielder.slot),
        });

        Self {
            yielder,
            body: body.boxed().fuse(),
        }
    }
}

/// The block's end of a [`TryAsyncStream`]: hands one value at a time to the stream.
pub struct Yielder<T> {
    // Locked only to move a value in or out, never across an await. A `Mutex` and not a
    // `RefCell` so that the stream is `Send`. It deliberately has no `Clone`.
    slot: Arc<Mutex<Option<T>>>,
}

impl<T> Yielder<T> {
    /// Hands `value` to the stream and suspends until the consumer polls again.
    ///
    /// Call it only inside the block given to [`TryAsyncStream::new`]: nothing schedules a
    /// wakeup, so anywhere else the task never resumes.
    pub async fn r#yield(&self, value: T) {
        let unread = self.slot().replace(value);
        debug_assert!(unread.is_none(), "BUG: the previous value was not taken");

        SuspendOnce(false).await
    }

    fn slot(&self) -> MutexGuard<'_, Option<T>> {
        self.slot
            .lock()
            .expect("BUG: a panic poisoned the stream's slot")
    }
}

/// Pending on its first poll, without waking the task, and ready on the next.
struct SuspendOnce(bool);

impl Future for SuspendOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        if mem::replace(&mut self.0, true) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl<T> Stream for TryAsyncStream<'_, T> {
    type Item = Result<T, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.body.is_terminated() {
            return Poll::Ready(None);
        }

        match self.body.poll_unpin(cx) {
            // The block is done; a yield would have left it pending.
            Poll::Ready(outcome) => Poll::Ready(outcome.err().map(Err)),
            // Pending on a yield, with its value in the slot, or on I/O that wakes the task.
            Poll::Pending => match self.yielder.slot().take() {
                Some(value) => Poll::Ready(Some(Ok(value))),
                None => Poll::Pending,
            },
        }
    }
}

/// Builds a [`TryAsyncStream`] from a block that calls `r#yield!(value)` once per item and
/// returns `Ok(())` at the end, or ends the stream early with an error (`?` included).
#[macro_export]
macro_rules! try_stream {
    ($($body:tt)*) => {
        $crate::ext::async_stream::TryAsyncStream::new(move |yielder| {
            ::tracing::Instrument::in_current_span(async move {
                // Only a borrow is in scope, so the yielder cannot move to another task.
                let yielder = &yielder;

                macro_rules! r#yield {
                    ($value:expr) => {{
                        yielder.r#yield($value).await;
                    }};
                }

                $($body)*
            })
        })
    };
}

#[cfg(test)]
#[path = "../../tests/unit/ext_async_stream_tests.rs"]
mod tests;
