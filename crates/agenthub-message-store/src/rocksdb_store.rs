//! RocksDB-backed [`MessageBodyStore`].
//!
//! Message bodies live in a dedicated `cf_body` column family compressed by RocksDB SST block
//! compression (zstd, with a higher zstd level on the bottommost level). Bodies are keyed by
//! [`AuthorityMessageId`], so one logical message stores exactly one body regardless of fan-out.
//!
//! This module is compiled only with the `rocksdb` feature so the default build does not pull the
//! native `librocksdb-sys` dependency.

use std::path::Path;
use std::sync::Mutex;

use rocksdb::{
    ColumnFamilyDescriptor, DB, DBCompressionType, Direction, IteratorMode, Options, WriteBatch,
    checkpoint::Checkpoint,
};

use crate::body_store::{BodyStoreError, MessageBodyStore};
use crate::ids::AuthorityMessageId;
use crate::index_store::{IndexStoreError, IndexedMessageRef, MessageIndexStore};
use crate::keys::{body_key, high_water_key};
use crate::reference::MessageRef;

/// Column family holding compressed message bodies.
pub const CF_BODY: &str = "cf_body";
/// Column family holding body-free delivery index refs.
pub const CF_INDEX: &str = "cf_index";

fn backend_err(err: impl ToString) -> BodyStoreError {
    BodyStoreError::Backend(err.to_string())
}

fn index_backend_err(err: impl ToString) -> IndexStoreError {
    IndexStoreError::Backend(err.to_string())
}

fn decode_high_water(bytes: &[u8]) -> Result<u64, IndexStoreError> {
    let raw: [u8; 8] = bytes.try_into().map_err(|_| {
        IndexStoreError::Backend(format!(
            "invalid high-water value length {}; expected 8",
            bytes.len()
        ))
    })?;
    Ok(u64::from_be_bytes(raw))
}

/// Build the `cf_body` options: zstd block compression, with a higher zstd level on the bottommost
/// level for cold data.
fn body_cf_options(compression: DBCompressionType) -> Options {
    let mut opts = Options::default();
    opts.set_compression_type(compression);
    if matches!(compression, DBCompressionType::Zstd) {
        opts.set_bottommost_compression_type(DBCompressionType::Zstd);
        // window_bits=-14 (default), level=19 (high), strategy=0, max_dict_bytes=0, enabled=true.
        opts.set_bottommost_compression_options(-14, 19, 0, 0, true);
    }
    opts
}

/// A RocksDB body store. Compression is handled by the storage engine; no body is compressed at the
/// application layer.
pub struct RocksdbBodyStore {
    db: DB,
    high_water_lock: Mutex<()>,
}

impl RocksdbBodyStore {
    /// Open (creating if needed) a body store at `path` with zstd compression on `cf_body`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BodyStoreError> {
        Self::open_with_compression(path, DBCompressionType::Zstd)
    }

    /// Open with an explicit compression type. Private: `open` is the public constructor; tests in
    /// the child module use this to compare on-disk size against an uncompressed store.
    fn open_with_compression(
        path: impl AsRef<Path>,
        compression: DBCompressionType,
    ) -> Result<Self, BodyStoreError> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let cfs = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_INDEX, Options::default()),
            ColumnFamilyDescriptor::new(CF_BODY, body_cf_options(compression)),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path, cfs).map_err(backend_err)?;
        Ok(Self {
            db,
            high_water_lock: Mutex::new(()),
        })
    }

    fn body_cf(&self) -> Result<&rocksdb::ColumnFamily, BodyStoreError> {
        self.db
            .cf_handle(CF_BODY)
            .ok_or_else(|| BodyStoreError::Backend(format!("missing column family {CF_BODY}")))
    }

    fn index_cf(&self) -> Result<&rocksdb::ColumnFamily, IndexStoreError> {
        self.db
            .cf_handle(CF_INDEX)
            .ok_or_else(|| IndexStoreError::Backend(format!("missing column family {CF_INDEX}")))
    }

    /// Atomically write one body and its derived delivery-index refs in a single RocksDB batch.
    pub fn put_body_and_refs<'a>(
        &self,
        id: &AuthorityMessageId,
        body: &[u8],
        refs: impl IntoIterator<Item = (&'a [u8], &'a MessageRef)>,
    ) -> Result<(), IndexStoreError> {
        let body_cf = self
            .body_cf()
            .map_err(|err| IndexStoreError::Backend(err.to_string()))?;
        let index_cf = self.index_cf()?;
        let mut batch = WriteBatch::default();
        batch.put_cf(body_cf, body_key(id), body);
        for (key, message_ref) in refs {
            batch.put_cf(index_cf, key, message_ref.to_bytes()?);
        }
        self.db.write(batch).map_err(index_backend_err)
    }

    /// Create a consistent RocksDB checkpoint containing all message-store column families.
    pub fn create_checkpoint(&self, path: impl AsRef<Path>) -> Result<(), BodyStoreError> {
        let checkpoint = Checkpoint::new(&self.db).map_err(backend_err)?;
        checkpoint.create_checkpoint(path).map_err(backend_err)
    }

    /// Flush memtables and compact `cf_body` so compression is applied to SST files. Test-only:
    /// it forces a full compaction and is only needed by the on-disk size test.
    #[cfg(test)]
    fn flush_and_compact(&self) -> Result<(), BodyStoreError> {
        let cf = self.body_cf()?;
        self.db.flush_cf(cf).map_err(backend_err)?;
        self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        Ok(())
    }
}

impl MessageBodyStore for RocksdbBodyStore {
    fn put_body(&self, id: &AuthorityMessageId, body: &[u8]) -> Result<(), BodyStoreError> {
        let cf = self.body_cf()?;
        self.db.put_cf(cf, body_key(id), body).map_err(backend_err)
    }

    fn get_body(&self, id: &AuthorityMessageId) -> Result<Option<Vec<u8>>, BodyStoreError> {
        let cf = self.body_cf()?;
        self.db.get_cf(cf, body_key(id)).map_err(backend_err)
    }

    fn contains(&self, id: &AuthorityMessageId) -> bool {
        // `get_pinned_cf` avoids copying the (potentially large) body just to test existence.
        self.body_cf()
            .ok()
            .and_then(|cf| self.db.get_pinned_cf(cf, body_key(id)).ok().flatten())
            .is_some()
    }

    fn len(&self) -> usize {
        // RocksDB has no O(1) key count; this scans cf_body. Diagnostics/test use only.
        match self.body_cf() {
            Ok(cf) => self.db.iterator_cf(cf, IteratorMode::Start).count(),
            Err(_) => 0,
        }
    }

    fn is_empty(&self) -> bool {
        // Avoid the O(N) default (`len() == 0`); checking for the first key is O(1).
        match self.body_cf() {
            Ok(cf) => self
                .db
                .iterator_cf(cf, IteratorMode::Start)
                .next()
                .is_none(),
            Err(_) => true,
        }
    }
}

impl MessageIndexStore for RocksdbBodyStore {
    fn put_ref(&self, key: &[u8], message_ref: &MessageRef) -> Result<(), IndexStoreError> {
        let cf = self.index_cf()?;
        self.db
            .put_cf(cf, key, message_ref.to_bytes()?)
            .map_err(index_backend_err)
    }

    fn delete_ref(&self, key: &[u8]) -> Result<(), IndexStoreError> {
        let cf = self.index_cf()?;
        self.db.delete_cf(cf, key).map_err(index_backend_err)
    }

    fn get_ref(&self, key: &[u8]) -> Result<Option<MessageRef>, IndexStoreError> {
        let cf = self.index_cf()?;
        let Some(bytes) = self.db.get_cf(cf, key).map_err(index_backend_err)? else {
            return Ok(None);
        };
        Ok(Some(MessageRef::from_bytes(&bytes)?))
    }

    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<MessageRef>, IndexStoreError> {
        let cf = self.index_cf()?;
        let mut refs = Vec::new();
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(prefix, Direction::Forward));
        for item in iter {
            let (key, value) = item.map_err(index_backend_err)?;
            if !key.starts_with(prefix) {
                break;
            }
            refs.push(MessageRef::from_bytes(&value)?);
        }
        Ok(refs)
    }

    fn scan_prefix_entries(
        &self,
        prefix: &[u8],
    ) -> Result<Vec<IndexedMessageRef>, IndexStoreError> {
        let cf = self.index_cf()?;
        let mut refs = Vec::new();
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(prefix, Direction::Forward));
        for item in iter {
            let (key, value) = item.map_err(index_backend_err)?;
            if !key.starts_with(prefix) {
                break;
            }
            refs.push(IndexedMessageRef {
                key: key.to_vec(),
                message_ref: MessageRef::from_bytes(&value)?,
            });
        }
        Ok(refs)
    }

    fn put_high_water(&self, stream_id: &str, seq: u64) -> Result<(), IndexStoreError> {
        let _guard = self
            .high_water_lock
            .lock()
            .expect("index high-water mutex poisoned");
        let cf = self.index_cf()?;
        let key = high_water_key(stream_id);
        let current = self.db.get_cf(cf, &key).map_err(index_backend_err)?;
        let next = match current {
            Some(bytes) => decode_high_water(&bytes)?.max(seq),
            None => seq,
        };
        self.db
            .put_cf(cf, key, next.to_be_bytes())
            .map_err(index_backend_err)
    }

    fn get_high_water(&self, stream_id: &str) -> Result<Option<u64>, IndexStoreError> {
        let cf = self.index_cf()?;
        self.db
            .get_cf(cf, high_water_key(stream_id))
            .map_err(index_backend_err)?
            .map(|bytes| decode_high_water(&bytes))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(repeat: usize) -> Vec<(AuthorityMessageId, String)> {
        let speakers = ["alice", "bob", "agent-claude", "agent-codex"];
        let phrases = [
            "Can you take a look at the failing test in the storage module?",
            "Sure, I'll check the dual-write path and report back.",
            "The transcript looks good, shipping the change now.",
            "Please rebase onto main and re-run the bazel crate tests.",
            "Done. CI is green and the PR is ready for review.",
        ];
        let mut out = Vec::new();
        for i in 0..repeat {
            let speaker = speakers[i % speakers.len()];
            let phrase = phrases[i % phrases.len()];
            let body = format!("{{\"speaker\":\"{speaker}\",\"seq\":{i},\"text\":\"{phrase}\"}}");
            out.push((AuthorityMessageId::new(format!("auth-{i}")), body));
        }
        out
    }

    /// Total size of the SST data files (`.sst`/`.ldb`) under `path`. Only these are subject to SST
    /// block compression, so restricting to them keeps the assertion about compression and ignores
    /// WAL/LOG/MANIFEST/OPTIONS noise.
    fn sst_size(path: &Path) -> u64 {
        let mut total = 0;
        let mut stack = vec![path.to_path_buf()];
        while let Some(p) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&p) else {
                continue;
            };
            for entry in entries.flatten() {
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if meta.is_dir() {
                    stack.push(entry.path());
                    continue;
                }
                let is_sst = entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "sst" || ext == "ldb");
                if is_sst {
                    total += meta.len();
                }
            }
        }
        total
    }

    fn write_compact_size(compression: DBCompressionType) -> u64 {
        let dir = tempfile::tempdir().expect("tempdir");
        {
            let store =
                RocksdbBodyStore::open_with_compression(dir.path(), compression).expect("open");
            for (id, body) in corpus(4000) {
                store.put_body(&id, body.as_bytes()).expect("put");
            }
            store.flush_and_compact().expect("compact");
        }
        sst_size(dir.path())
    }

    #[test]
    fn body_round_trips_through_cf_body() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        let id = AuthorityMessageId::new("auth-1");
        let body = b"a meeting summary body";
        store.put_body(&id, body).unwrap();
        assert_eq!(store.get_body(&id).unwrap().as_deref(), Some(&body[..]));
        assert!(store.contains(&id));
        let missing = AuthorityMessageId::new("auth-missing");
        assert_eq!(store.get_body(&missing).unwrap(), None);
        assert!(!store.contains(&missing));
    }

    fn sample_ref(seq: u64) -> MessageRef {
        MessageRef {
            message_id: crate::DeliveryMessageId::new(format!("delivery-{seq}")),
            authority_message_id: AuthorityMessageId::new(format!("auth-{seq}")),
            archive_document_id: Some(format!("doc-{seq}")),
            created_at: 1_700_000_000 + seq as i64,
            source_kind: "team_conversation_messages".to_string(),
            message_kind: crate::MessageKind::Text,
            correlation_id: None,
            group_id: Some("group-a".to_string()),
            run_id: Some("run-a".to_string()),
            conversation_id: Some("chan-1".to_string()),
            agent_id: Some("agent-a".to_string()),
        }
    }

    #[test]
    fn index_refs_round_trip_through_cf_index_in_prefix_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        let first = sample_ref(1);
        let second = sample_ref(2);

        store
            .put_ref(&crate::keys::channel_key("group-a", "chan-1", 1), &first)
            .unwrap();
        store
            .put_ref(&crate::keys::channel_key("group-a", "chan-1", 2), &second)
            .unwrap();

        assert_eq!(
            store
                .scan_prefix(&crate::keys::channel_prefix("group-a", "chan-1"))
                .unwrap(),
            vec![first.clone(), second]
        );
        assert_eq!(
            store
                .get_ref(&crate::keys::channel_key("group-a", "chan-1", 1))
                .unwrap(),
            Some(first)
        );
    }

    #[test]
    fn body_and_index_refs_write_in_one_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        let id = AuthorityMessageId::new("auth-batch");
        let message_ref = MessageRef {
            authority_message_id: id.clone(),
            ..sample_ref(9)
        };
        let channel_key = crate::keys::channel_key("group-a", "chan-1", 9);
        let id_key = crate::keys::message_id_key(message_ref.message_id.as_str());

        store
            .put_body_and_refs(
                &id,
                b"batch body",
                [
                    (channel_key.as_slice(), &message_ref),
                    (id_key.as_slice(), &message_ref),
                ],
            )
            .unwrap();

        assert_eq!(
            store.get_body(&id).unwrap().as_deref(),
            Some(&b"batch body"[..])
        );
        assert_eq!(store.get_ref(&id_key).unwrap(), Some(message_ref));
    }

    #[test]
    fn high_water_round_trips_through_cf_index_without_regressing() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();

        assert_eq!(store.get_high_water("team_actor_messages").unwrap(), None);
        store.put_high_water("team_actor_messages", 11).unwrap();
        assert_eq!(
            store.get_high_water("team_actor_messages").unwrap(),
            Some(11)
        );

        store.put_high_water("team_actor_messages", 7).unwrap();
        assert_eq!(
            store.get_high_water("team_actor_messages").unwrap(),
            Some(11)
        );
    }

    #[test]
    fn index_body_integrity_scans_cf_index_and_cf_body() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        let present_id = AuthorityMessageId::new("auth-present");
        let missing_id = AuthorityMessageId::new("auth-missing");
        store.put_body(&present_id, b"present body").unwrap();

        let present_ref = MessageRef {
            authority_message_id: present_id,
            ..sample_ref(1)
        };
        let missing_ref = MessageRef {
            authority_message_id: missing_id.clone(),
            ..sample_ref(2)
        };
        store
            .put_ref(
                &crate::keys::channel_key("group-a", "chan-1", 1),
                &present_ref,
            )
            .unwrap();
        store
            .put_ref(
                &crate::keys::channel_key("group-a", "chan-1", 2),
                &missing_ref,
            )
            .unwrap();

        let report = crate::check_index_refs_have_bodies(
            &store,
            &store,
            [crate::keys::channel_prefix("group-a", "chan-1")],
        )
        .unwrap();

        assert_eq!(report.scanned_refs, 2);
        assert_eq!(report.missing_body_refs.len(), 1);
        assert_eq!(report.missing_authority_message_ids, vec![missing_id]);
    }

    #[test]
    fn index_authority_integrity_reports_orphan_cf_index_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        let authority_id = AuthorityMessageId::new("auth-present");
        let orphan_id = AuthorityMessageId::new("auth-orphan");
        let authority_ref = MessageRef {
            authority_message_id: authority_id.clone(),
            ..sample_ref(1)
        };
        let orphan_ref = MessageRef {
            authority_message_id: orphan_id.clone(),
            ..sample_ref(2)
        };
        store
            .put_ref(&crate::keys::run_key("run-a", 1), &authority_ref)
            .unwrap();
        store
            .put_ref(&crate::keys::run_key("run-a", 2), &orphan_ref)
            .unwrap();

        let report = crate::check_index_refs_have_authority(
            &store,
            [crate::keys::run_prefix("run-a")],
            [authority_id],
        )
        .unwrap();

        assert_eq!(report.scanned_refs, 2);
        assert_eq!(report.orphan_refs.len(), 1);
        assert_eq!(report.orphan_authority_message_ids, vec![orphan_id]);
    }

    #[test]
    fn index_authority_prune_deletes_orphan_cf_index_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        let authority_id = AuthorityMessageId::new("auth-present");
        let orphan_id = AuthorityMessageId::new("auth-orphan");
        let authority_ref = MessageRef {
            authority_message_id: authority_id.clone(),
            ..sample_ref(1)
        };
        let orphan_ref = MessageRef {
            authority_message_id: orphan_id.clone(),
            ..sample_ref(2)
        };
        let authority_key = crate::keys::run_key("run-a", 1);
        let orphan_key = crate::keys::run_key("run-a", 2);

        store.put_ref(&authority_key, &authority_ref).unwrap();
        store.put_ref(&orphan_key, &orphan_ref).unwrap();
        store.put_high_water("team_run_events", 2).unwrap();

        let report = crate::prune_index_refs_without_authority(
            &store,
            [crate::keys::run_prefix("run-a")],
            [authority_id],
        )
        .unwrap();

        assert_eq!(report.scanned_refs, 2);
        assert_eq!(report.pruned_refs.len(), 1);
        assert_eq!(report.pruned_refs[0].index_key, orphan_key);
        assert_eq!(report.pruned_authority_message_ids, vec![orphan_id]);
        assert_eq!(store.get_ref(&authority_key).unwrap(), Some(authority_ref));
        assert_eq!(store.get_ref(&orphan_key).unwrap(), None);
        assert_eq!(store.get_high_water("team_run_events").unwrap(), Some(2));
    }

    #[test]
    fn checkpoint_restore_preserves_cf_body_cf_index_and_high_water() {
        let source_dir = tempfile::tempdir().unwrap();
        let checkpoint_dir = tempfile::tempdir().unwrap();
        let backup_path = checkpoint_dir.path().join("message-store-backup");
        let authority_id = AuthorityMessageId::new("auth-backup");
        let message_ref = MessageRef {
            authority_message_id: authority_id.clone(),
            ..sample_ref(42)
        };
        let channel_key = crate::keys::channel_key("group-a", "chan-1", 42);
        let id_key = crate::keys::message_id_key(message_ref.message_id.as_str());

        {
            let store = RocksdbBodyStore::open(source_dir.path()).unwrap();
            store
                .put_body_and_refs(
                    &authority_id,
                    b"body that must survive backup restore",
                    [
                        (channel_key.as_slice(), &message_ref),
                        (id_key.as_slice(), &message_ref),
                    ],
                )
                .unwrap();
            store
                .put_high_water("team_conversation_messages", 42)
                .unwrap();
            store.create_checkpoint(&backup_path).unwrap();
        }

        let restored = RocksdbBodyStore::open(&backup_path).unwrap();
        assert_eq!(
            restored.get_body(&authority_id).unwrap().as_deref(),
            Some(&b"body that must survive backup restore"[..])
        );
        assert_eq!(
            restored
                .scan_prefix(&crate::keys::channel_prefix("group-a", "chan-1"))
                .unwrap(),
            vec![message_ref.clone()]
        );
        assert_eq!(restored.get_ref(&id_key).unwrap(), Some(message_ref));
        assert_eq!(
            restored
                .get_high_water("team_conversation_messages")
                .unwrap(),
            Some(42)
        );
    }

    #[test]
    fn checkpoint_restore_survives_source_loss_and_passes_integrity_checks() {
        let source_dir = tempfile::tempdir().unwrap();
        let checkpoint_dir = tempfile::tempdir().unwrap();
        let backup_path = checkpoint_dir.path().join("message-store-backup");

        let present_id = AuthorityMessageId::new("auth-backup-present");
        let present_ref = MessageRef {
            authority_message_id: present_id.clone(),
            ..sample_ref(42)
        };
        let channel_key = crate::keys::channel_key("group-a", "chan-1", 42);
        let id_key = crate::keys::message_id_key(present_ref.message_id.as_str());

        {
            let store = RocksdbBodyStore::open(source_dir.path()).unwrap();
            store
                .put_body_and_refs(
                    &present_id,
                    b"body that must survive total source loss",
                    [
                        (channel_key.as_slice(), &present_ref),
                        (id_key.as_slice(), &present_ref),
                    ],
                )
                .unwrap();
            store
                .put_high_water("team_conversation_messages", 42)
                .unwrap();
            store.create_checkpoint(&backup_path).unwrap();
        }

        // Disaster-recovery evidence: the original store is gone entirely, so the checkpoint is the
        // only remaining copy. If restore silently depended on the source directory, this would fail to
        // open or return stale/empty data instead of the checkpointed state.
        std::fs::remove_dir_all(source_dir.path()).unwrap();

        let restored = RocksdbBodyStore::open(&backup_path).unwrap();
        assert_eq!(
            restored.get_body(&present_id).unwrap().as_deref(),
            Some(&b"body that must survive total source loss"[..])
        );

        // The restored copy must itself pass the same integrity gates an operator would run before
        // trusting it: every index ref resolves to a body, and every index ref is backed by authority.
        // These are the two integrity primitives from `crate::integrity` that previously had no caller
        // outside their own isolated unit tests.
        let body_report = crate::check_index_refs_have_bodies(
            &restored,
            &restored,
            [crate::keys::channel_prefix("group-a", "chan-1")],
        )
        .unwrap();
        assert!(
            body_report.is_clean(),
            "restored store must have no index refs missing a body: {:?}",
            body_report.missing_body_refs
        );

        let authority_report = crate::check_index_refs_have_authority(
            &restored,
            [crate::keys::channel_prefix("group-a", "chan-1")],
            [present_id],
        )
        .unwrap();
        assert!(
            authority_report.is_clean(),
            "restored store must have no orphan index refs: {:?}",
            authority_report.orphan_refs
        );
    }

    #[test]
    fn fan_out_stores_one_body_per_authority_message() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        let id = AuthorityMessageId::new("auth-fanout");
        for _delivery in 0..5 {
            store.put_body(&id, b"shared body").unwrap();
        }
        assert_eq!(store.len(), 1, "fan-out must not duplicate the body");
    }

    #[test]
    fn zstd_compression_reduces_on_disk_size() {
        // The same chat data must take less disk with zstd than with no compression, proving SST
        // block compression is active and helps on real chat-like bodies.
        let zstd_size = write_compact_size(DBCompressionType::Zstd);
        let none_size = write_compact_size(DBCompressionType::None);
        assert!(
            zstd_size < none_size,
            "zstd SST size ({zstd_size}) must be smaller than uncompressed ({none_size})"
        );
    }
}
