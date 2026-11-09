//! The guard that gives a pool's size and semaphore permit back when a connection is lost.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::inner::PoolInner;
use crate::database::Database;
use crate::sync::AsyncSemaphoreReleaser;

/// RAII guard returned by `Pool::try_increment_size()` and others.
///
/// Will decrement the pool size if dropped, to avoid semantically "leaking" connections
/// (where the pool thinks it has more connections than it does).
pub(in crate::pool) struct DecrementSizeGuard<DB: Database> {
    pub(crate) pool: Arc<PoolInner<DB>>,
    cancelled: bool,
}

impl<DB: Database> DecrementSizeGuard<DB> {
    /// Create a new guard that will release a semaphore permit on-drop.
    pub fn new_permit(pool: Arc<PoolInner<DB>>) -> Self {
        Self {
            pool,
            cancelled: false,
        }
    }

    pub fn from_permit(pool: Arc<PoolInner<DB>>, permit: AsyncSemaphoreReleaser<'_>) -> Self {
        // here we effectively take ownership of the permit
        permit.disarm();
        Self::new_permit(pool)
    }

    /// Release the semaphore permit without decreasing the pool size.
    pub(super) fn release_permit(self) {
        self.pool.semaphore.release(1);
        self.cancel();
    }

    pub fn cancel(mut self) {
        self.cancelled = true;
    }
}

impl<DB: Database> Drop for DecrementSizeGuard<DB> {
    fn drop(&mut self) {
        if !self.cancelled {
            self.pool.size.fetch_sub(1, Ordering::AcqRel);

            // and here we release the permit we got on construction
            self.pool.semaphore.release(1);
        }
    }
}
