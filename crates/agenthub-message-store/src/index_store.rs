//! Body-free delivery index abstraction and repair helpers.
//!
//! The index is a derived projection: every row can be rebuilt from SQLite authority metadata. This
//! module intentionally does not depend on the body store, so repair code cannot accidentally treat
//! `cf_body` as an input to index reconstruction.

use std::collections::BTreeMap;
use std::sync::Mutex;

use thiserror::Error;

use crate::reference::MessageRef;

#[derive(Debug, Error)]
pub enum IndexStoreError {
    #[error("message index backend failure: {0}")]
    Backend(String),
    #[error("message index row codec failure: {0}")]
    Codec(#[from] serde_json::Error),
}

/// Stores body-free delivery refs keyed by deterministic ordered index keys.
pub trait MessageIndexStore: Send + Sync {
    /// Put one body-free ref at `key`. Replaying the same authority projection is idempotent.
    fn put_ref(&self, key: &[u8], message_ref: &MessageRef) -> Result<(), IndexStoreError>;

    /// Delete one body-free ref by exact key.
    fn delete_ref(&self, key: &[u8]) -> Result<(), IndexStoreError> {
        let _ = key;
        Err(IndexStoreError::Backend(
            "message index backend does not support ref deletion".to_string(),
        ))
    }

    /// Fetch one ref by its exact index key.
    fn get_ref(&self, key: &[u8]) -> Result<Option<MessageRef>, IndexStoreError>;

    /// Scan refs whose keys start with `prefix`, in bytewise key order.
    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<MessageRef>, IndexStoreError>;

    /// Scan refs with their exact keys. Prune operations use the key to delete only explicit orphans.
    fn scan_prefix_entries(
        &self,
        prefix: &[u8],
    ) -> Result<Vec<IndexedMessageRef>, IndexStoreError> {
        let _ = prefix;
        Err(IndexStoreError::Backend(
            "message index backend does not support keyed prefix scans".to_string(),
        ))
    }

    /// Record the highest SQLite authority row id projected for `stream_id`.
    ///
    /// Implementations must keep this marker monotonic: replaying an older repair pass must not
    /// lower the high-water mark.
    fn put_high_water(&self, stream_id: &str, seq: u64) -> Result<(), IndexStoreError>;

    /// Fetch the highest SQLite authority row id projected for `stream_id`.
    fn get_high_water(&self, stream_id: &str) -> Result<Option<u64>, IndexStoreError>;
}

/// One deterministic index row derived from SQLite authority metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityIndexProjection {
    pub key: Vec<u8>,
    pub message_ref: MessageRef,
}

/// One index row plus its exact key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedMessageRef {
    pub key: Vec<u8>,
    pub message_ref: MessageRef,
}

/// Summary of an authority-derived index repair run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexRepairReport {
    pub repaired_refs: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexFreshness {
    Fresh {
        indexed_through: u64,
    },
    Lagging {
        indexed_through: Option<u64>,
        authority_max: u64,
    },
}

impl IndexFreshness {
    pub fn is_fresh(&self) -> bool {
        matches!(self, Self::Fresh { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexReadRepairReason {
    Lagging { indexed_through: Option<u64> },
    Incomplete { indexed_through: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexReadRepairRequest {
    pub stream_id: String,
    pub authority_max: u64,
    pub reason: IndexReadRepairReason,
}

/// Records index streams that should be repaired after a guarded read falls back to authority.
///
/// Scheduling must be non-invasive: callers still serve SQLite immediately. Implementations should
/// keep the highest requested authority bound per stream so repeated lagging reads do not regress the
/// target repair watermark.
pub trait IndexReadRepairScheduler: Send + Sync {
    fn schedule_read_repair(&self, request: IndexReadRepairRequest) -> Result<(), IndexStoreError>;

    /// Atomically claim the currently scheduled repairs.
    ///
    /// A worker owns the returned batch. Failed work must be scheduled again so a concurrent
    /// guarded read can raise its authority bound while the worker is rebuilding the projection.
    fn take_pending_repairs(&self) -> Result<Vec<IndexReadRepairRequest>, IndexStoreError>;
}

/// In-memory read-repair scheduler used by the single-process Phase 1 runtime and tests.
#[derive(Debug, Default)]
pub struct InMemoryIndexReadRepairScheduler {
    requests: Mutex<BTreeMap<String, IndexReadRepairRequest>>,
}

impl InMemoryIndexReadRepairScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pending_repairs(&self) -> Vec<IndexReadRepairRequest> {
        self.requests
            .lock()
            .expect("index read-repair scheduler mutex poisoned")
            .values()
            .cloned()
            .collect()
    }

    pub fn take_pending_repairs(&self) -> Vec<IndexReadRepairRequest> {
        let mut requests = self
            .requests
            .lock()
            .expect("index read-repair scheduler mutex poisoned");
        std::mem::take(&mut *requests).into_values().collect()
    }
}

impl IndexReadRepairScheduler for InMemoryIndexReadRepairScheduler {
    fn schedule_read_repair(&self, request: IndexReadRepairRequest) -> Result<(), IndexStoreError> {
        let mut requests = self
            .requests
            .lock()
            .expect("index read-repair scheduler mutex poisoned");
        let entry = requests
            .entry(request.stream_id.clone())
            .or_insert_with(|| request.clone());
        if request.authority_max > entry.authority_max {
            *entry = request;
        }
        Ok(())
    }

    fn take_pending_repairs(&self) -> Result<Vec<IndexReadRepairRequest>, IndexStoreError> {
        Ok(self.take_pending_repairs())
    }
}

/// Rebuild missing/stale delivery refs from authority-derived projections.
///
/// The function only writes the index store. It never reads `cf_body`, because body bytes are not
/// part of the source of truth for a delivery-index rebuild.
pub fn repair_index_from_authority<I, P>(
    index: &I,
    projections: P,
) -> Result<IndexRepairReport, IndexStoreError>
where
    I: MessageIndexStore + ?Sized,
    P: IntoIterator<Item = AuthorityIndexProjection>,
{
    let mut report = IndexRepairReport::default();
    for projection in projections {
        index.put_ref(&projection.key, &projection.message_ref)?;
        report.repaired_refs += 1;
    }
    Ok(report)
}

/// Rebuild authority-derived refs, then mark the projection stream repaired through `authority_max`.
///
/// The high-water mark is advanced only after all refs have been written. This helper is the basic
/// read-repair building block; production ordered reads still need a caller-level fallback policy
/// before they can prefer `cf_index` over SQLite.
pub fn repair_index_from_authority_through<I, P>(
    index: &I,
    projections: P,
    stream_id: &str,
    authority_max: u64,
) -> Result<IndexRepairReport, IndexStoreError>
where
    I: MessageIndexStore + ?Sized,
    P: IntoIterator<Item = AuthorityIndexProjection>,
{
    let report = repair_index_from_authority(index, projections)?;
    mark_index_repaired_through(index, stream_id, authority_max)?;
    Ok(report)
}

/// Mark a projection stream repaired through `authority_max`.
pub fn mark_index_repaired_through<I>(
    index: &I,
    stream_id: &str,
    authority_max: u64,
) -> Result<(), IndexStoreError>
where
    I: MessageIndexStore + ?Sized,
{
    index.put_high_water(stream_id, authority_max)
}

/// Guard an indexed read by comparing its projection high-water mark with SQLite authority.
pub fn check_index_freshness<I>(
    index: &I,
    stream_id: &str,
    authority_max: u64,
) -> Result<IndexFreshness, IndexStoreError>
where
    I: MessageIndexStore + ?Sized,
{
    let indexed_through = index.get_high_water(stream_id)?;
    Ok(match indexed_through {
        Some(indexed_through) if indexed_through >= authority_max => {
            IndexFreshness::Fresh { indexed_through }
        }
        indexed_through => IndexFreshness::Lagging {
            indexed_through,
            authority_max,
        },
    })
}

/// In-memory reference implementation used by tests and repair contracts.
#[derive(Debug, Default)]
pub struct InMemoryIndexStore {
    refs: Mutex<BTreeMap<Vec<u8>, MessageRef>>,
    high_water: Mutex<BTreeMap<String, u64>>,
}

impl InMemoryIndexStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MessageIndexStore for InMemoryIndexStore {
    fn put_ref(&self, key: &[u8], message_ref: &MessageRef) -> Result<(), IndexStoreError> {
        self.refs
            .lock()
            .expect("index store mutex poisoned")
            .insert(key.to_vec(), message_ref.clone());
        Ok(())
    }

    fn delete_ref(&self, key: &[u8]) -> Result<(), IndexStoreError> {
        self.refs
            .lock()
            .expect("index store mutex poisoned")
            .remove(key);
        Ok(())
    }

    fn get_ref(&self, key: &[u8]) -> Result<Option<MessageRef>, IndexStoreError> {
        Ok(self
            .refs
            .lock()
            .expect("index store mutex poisoned")
            .get(key)
            .cloned())
    }

    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<MessageRef>, IndexStoreError> {
        let refs = self.refs.lock().expect("index store mutex poisoned");
        Ok(refs
            .range(prefix.to_vec()..)
            .take_while(|(key, _)| key.starts_with(prefix))
            .map(|(_, message_ref)| message_ref.clone())
            .collect())
    }

    fn scan_prefix_entries(
        &self,
        prefix: &[u8],
    ) -> Result<Vec<IndexedMessageRef>, IndexStoreError> {
        let refs = self.refs.lock().expect("index store mutex poisoned");
        Ok(refs
            .range(prefix.to_vec()..)
            .take_while(|(key, _)| key.starts_with(prefix))
            .map(|(key, message_ref)| IndexedMessageRef {
                key: key.clone(),
                message_ref: message_ref.clone(),
            })
            .collect())
    }

    fn put_high_water(&self, stream_id: &str, seq: u64) -> Result<(), IndexStoreError> {
        let mut high_water = self
            .high_water
            .lock()
            .expect("index high-water mutex poisoned");
        let current = high_water.entry(stream_id.to_string()).or_insert(0);
        *current = (*current).max(seq);
        Ok(())
    }

    fn get_high_water(&self, stream_id: &str) -> Result<Option<u64>, IndexStoreError> {
        Ok(self
            .high_water
            .lock()
            .expect("index high-water mutex poisoned")
            .get(stream_id)
            .copied())
    }
}
