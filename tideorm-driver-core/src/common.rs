//! Small helpers the backends share: a connection's prepared-statement cache, and a
//! `Debug` stand-in for closures.

use hashlink::lru_cache::LruCache;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};

/// A closure in a struct that derives `Debug`; it prints as `Function`.
pub struct DebugFn<F: ?Sized>(pub F);

impl<F: ?Sized> Deref for DebugFn<F> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F: ?Sized> DerefMut for DebugFn<F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<F: ?Sized> Debug for DebugFn<F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Function").finish()
    }
}

/// A connection's prepared statements, keyed by their SQL, holding at most `capacity`;
/// adding one to a full cache evicts the least recently used.
///
/// The cache hands back what it evicts or replaces, so the caller can close that
/// statement on the server.
#[derive(Debug)]
pub struct StatementCache<T> {
    inner: LruCache<String, T>,
}

impl<T> StatementCache<T> {
    /// An empty cache for `capacity` statements; 0 caches none.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: LruCache::new(capacity),
        }
    }

    /// The statement cached for `k`, which becomes the most recently used.
    pub fn get_mut(&mut self, k: &str) -> Option<&mut T> {
        self.inner.get_mut(k)
    }

    /// Caches `v` for `k` as the most recently used statement. Returns the statement it
    /// displaced: the one cached for `k` before, or else the least recently used one
    /// when the cache was full.
    pub fn insert(&mut self, k: &str, v: T) -> Option<T> {
        // Evict here rather than in `LruCache::insert`, which drops what it evicts.
        let evicted = if self.len() == self.capacity() && !self.contains_key(k) {
            self.remove_lru()
        } else {
            None
        };

        self.inner.insert(k.into(), v).or(evicted)
    }

    /// How many statements are cached.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether no statement is cached.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Takes the least recently used statement out of the cache.
    pub fn remove_lru(&mut self) -> Option<T> {
        self.inner.remove_lru().map(|(_, v)| v)
    }

    /// Empties the cache.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Whether a statement is cached for `k`.
    pub fn contains_key(&mut self, k: &str) -> bool {
        self.inner.contains_key(k)
    }

    /// The most statements the cache holds.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Whether the cache holds anything at all (its capacity is not 0).
    pub fn is_enabled(&self) -> bool {
        self.capacity() > 0
    }
}

#[cfg(test)]
#[path = "../tests/unit/common_tests.rs"]
mod tests;
