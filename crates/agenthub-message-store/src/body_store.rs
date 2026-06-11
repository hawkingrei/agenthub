//! The message body store abstraction.
//!
//! The body store is keyed by [`AuthorityMessageId`], so one logical message stores exactly one body
//! even when it fans out to many actors/channels. This is the backend-agnostic boundary the
//! storage-tiering spec requires: the production backend is RocksDB (compression handled by SST block
//! compression), but the trait keeps callers and tests independent of it.
//!
//! Methods take `&self` so one store can be shared (e.g. behind an `Arc`) between the background
//! drainer that writes bodies and the read path that fetches them; backends use interior mutability.

use std::collections::BTreeMap;
use std::sync::Mutex;

use thiserror::Error;

use crate::ids::AuthorityMessageId;

#[derive(Debug, Error)]
pub enum BodyStoreError {
    #[error("body store backend failure: {0}")]
    Backend(String),
}

/// Stores and retrieves message bodies keyed by the canonical logical-message identity.
pub trait MessageBodyStore: Send + Sync {
    /// Store the body for `id`. Idempotent for a given logical message: a logical message has one
    /// immutable body, so re-putting the same id is a no-op-equivalent overwrite and never creates a
    /// second copy.
    fn put_body(&self, id: &AuthorityMessageId, body: &[u8]) -> Result<(), BodyStoreError>;

    /// Fetch the body for `id`, or `None` if absent.
    fn get_body(&self, id: &AuthorityMessageId) -> Result<Option<Vec<u8>>, BodyStoreError>;

    /// Whether a body exists for `id`.
    fn contains(&self, id: &AuthorityMessageId) -> bool;

    /// Number of distinct logical-message bodies held.
    fn len(&self) -> usize;

    /// Whether the store holds no bodies.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// In-memory reference implementation used by tests and as the abstraction's executable contract.
#[derive(Debug, Default)]
pub struct InMemoryBodyStore {
    bodies: Mutex<BTreeMap<AuthorityMessageId, Vec<u8>>>,
}

impl InMemoryBodyStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MessageBodyStore for InMemoryBodyStore {
    fn put_body(&self, id: &AuthorityMessageId, body: &[u8]) -> Result<(), BodyStoreError> {
        self.bodies
            .lock()
            .expect("body store mutex poisoned")
            .insert(id.clone(), body.to_vec());
        Ok(())
    }

    fn get_body(&self, id: &AuthorityMessageId) -> Result<Option<Vec<u8>>, BodyStoreError> {
        Ok(self
            .bodies
            .lock()
            .expect("body store mutex poisoned")
            .get(id)
            .cloned())
    }

    fn contains(&self, id: &AuthorityMessageId) -> bool {
        self.bodies
            .lock()
            .expect("body store mutex poisoned")
            .contains_key(id)
    }

    fn len(&self) -> usize {
        self.bodies.lock().expect("body store mutex poisoned").len()
    }
}

/// A body store wrapper that fails the next `n` writes, used to exercise the outbox replay path.
#[cfg(test)]
#[derive(Debug)]
pub struct FailingBodyStore {
    inner: InMemoryBodyStore,
    fail_next: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl FailingBodyStore {
    pub fn new(fail_next: usize) -> Self {
        Self {
            inner: InMemoryBodyStore::new(),
            fail_next: std::sync::atomic::AtomicUsize::new(fail_next),
        }
    }
}

#[cfg(test)]
impl MessageBodyStore for FailingBodyStore {
    fn put_body(&self, id: &AuthorityMessageId, body: &[u8]) -> Result<(), BodyStoreError> {
        use std::sync::atomic::Ordering;
        // Atomically check-and-decrement so concurrent callers can't underflow the counter.
        if self
            .fail_next
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                (v > 0).then(|| v - 1)
            })
            .is_ok()
        {
            return Err(BodyStoreError::Backend("injected failure".to_string()));
        }
        self.inner.put_body(id, body)
    }

    fn get_body(&self, id: &AuthorityMessageId) -> Result<Option<Vec<u8>>, BodyStoreError> {
        self.inner.get_body(id)
    }

    fn contains(&self, id: &AuthorityMessageId) -> bool {
        self.inner.contains(id)
    }

    fn len(&self) -> usize {
        self.inner.len()
    }
}
