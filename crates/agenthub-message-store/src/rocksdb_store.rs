//! RocksDB-backed [`MessageBodyStore`].
//!
//! Message bodies live in a dedicated `cf_body` column family compressed by RocksDB SST block
//! compression (zstd, with a higher zstd level on the bottommost level). Bodies are keyed by
//! [`AuthorityMessageId`], so one logical message stores exactly one body regardless of fan-out.
//!
//! This module is compiled only with the `rocksdb` feature so the default build does not pull the
//! native `librocksdb-sys` dependency.

use std::path::Path;

use rocksdb::{
    ColumnFamilyDescriptor, DB, DBCompressionType, Direction, IteratorMode, Options, WriteBatch,
};

use crate::body_store::{BodyStoreError, MessageBodyStore};
use crate::ids::AuthorityMessageId;
use crate::keys::body_key;
use crate::{
    DeliveryMessageId, MessageRef,
    index_store::{MessageIndex, MessageIndexError, MessageIndexProjection},
    keys,
};

/// Column family holding compressed message bodies.
pub const CF_BODY: &str = "cf_body";
/// Column family holding body-free delivery index rows.
pub const CF_INDEX: &str = "cf_index";

fn backend_err(err: impl ToString) -> BodyStoreError {
    BodyStoreError::Backend(err.to_string())
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
            ColumnFamilyDescriptor::new(CF_BODY, body_cf_options(compression)),
            ColumnFamilyDescriptor::new(CF_INDEX, Options::default()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path, cfs).map_err(backend_err)?;
        Ok(Self { db })
    }

    fn body_cf(&self) -> Result<&rocksdb::ColumnFamily, BodyStoreError> {
        self.db
            .cf_handle(CF_BODY)
            .ok_or_else(|| BodyStoreError::Backend(format!("missing column family {CF_BODY}")))
    }

    fn index_cf(&self) -> Result<&rocksdb::ColumnFamily, MessageIndexError> {
        self.db
            .cf_handle(CF_INDEX)
            .ok_or_else(|| MessageIndexError::Backend(format!("missing column family {CF_INDEX}")))
    }

    /// Put the body and all derived delivery-index refs in one RocksDB batch.
    pub fn put_body_and_index(
        &self,
        id: &AuthorityMessageId,
        body: &[u8],
        projection: &MessageIndexProjection,
    ) -> Result<(), MessageIndexError> {
        let body_cf = self
            .body_cf()
            .map_err(|err| MessageIndexError::Backend(err.to_string()))?;
        let index_cf = self.index_cf()?;
        let mut batch = WriteBatch::default();
        batch.put_cf(body_cf, body_key(id), body);
        for (key, value) in projection.entries()? {
            batch.put_cf(index_cf, key, value);
        }
        self.db
            .write(batch)
            .map_err(|err| MessageIndexError::Backend(err.to_string()))
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

impl RocksdbBodyStore {
    fn decode_index_ref(bytes: &[u8]) -> Result<MessageRef, MessageIndexError> {
        MessageRef::from_bytes(bytes).map_err(MessageIndexError::Encode)
    }
}

impl MessageIndex for RocksdbBodyStore {
    fn put_message(&self, projection: &MessageIndexProjection) -> Result<(), MessageIndexError> {
        let cf = self.index_cf()?;
        let mut batch = WriteBatch::default();
        for (key, value) in projection.entries()? {
            batch.put_cf(cf, key, value);
        }
        self.db
            .write(batch)
            .map_err(|err| MessageIndexError::Backend(err.to_string()))
    }

    fn get_by_id(
        &self,
        message_id: &DeliveryMessageId,
    ) -> Result<Option<MessageRef>, MessageIndexError> {
        let cf = self.index_cf()?;
        self.db
            .get_cf(cf, keys::by_id_key(message_id))
            .map_err(|err| MessageIndexError::Backend(err.to_string()))?
            .map(|bytes| Self::decode_index_ref(&bytes))
            .transpose()
    }

    fn scan_prefix(
        &self,
        prefix: &[u8],
        limit: usize,
    ) -> Result<Vec<MessageRef>, MessageIndexError> {
        let cf = self.index_cf()?;
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(prefix, Direction::Forward));
        let mut refs = Vec::new();
        for item in iter {
            let (key, value) = item.map_err(|err| MessageIndexError::Backend(err.to_string()))?;
            if !key.starts_with(prefix) || refs.len() >= limit {
                break;
            }
            refs.push(Self::decode_index_ref(&value)?);
        }
        Ok(refs)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeliveryMessageId, MessageIndex, MessageIndexProjection, MessageKind, MessageRef};

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

    fn sample_ref(seq: u64) -> MessageRef {
        MessageRef {
            message_id: DeliveryMessageId::new(format!("delivery-{seq}")),
            authority_message_id: AuthorityMessageId::new(format!("auth-{seq}")),
            archive_document_id: Some(format!("doc-{seq}")),
            created_at: 1_700_000_000 + seq as i64,
            source_kind: "team_conversation_messages".to_string(),
            message_kind: MessageKind::Text,
            correlation_id: Some(format!("corr-{seq}")),
            group_id: Some("group-a".to_string()),
            run_id: Some("run-a".to_string()),
            conversation_id: Some("channel-a".to_string()),
            agent_id: Some("agent-a".to_string()),
        }
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

    #[test]
    fn cf_index_scans_ordered_body_free_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        for seq in [2, 0, 1] {
            let reference = sample_ref(seq);
            let projection = MessageIndexProjection::new(seq, reference)
                .for_channel("group-a", "channel-a")
                .for_agent("agent-a")
                .for_run("run-a")
                .for_inbox_actor("actor-a");
            store.put_message(&projection).unwrap();
        }

        let channel = store.scan_channel("group-a", "channel-a", 10).unwrap();
        let ids: Vec<_> = channel.iter().map(|row| row.message_id.as_str()).collect();
        assert_eq!(ids, ["delivery-0", "delivery-1", "delivery-2"]);
        assert_eq!(
            store
                .get_by_id(&DeliveryMessageId::new("delivery-2"))
                .unwrap()
                .unwrap()
                .authority_message_id
                .as_str(),
            "auth-2"
        );
        assert_eq!(store.scan_agent("agent-a", 10).unwrap().len(), 3);
        assert_eq!(store.scan_run("run-a", 10).unwrap().len(), 3);
        assert_eq!(store.scan_inbox("actor-a", 1).unwrap().len(), 1);
    }

    #[test]
    fn body_and_index_write_share_one_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksdbBodyStore::open(dir.path()).unwrap();
        let reference = sample_ref(7);
        let authority = reference.authority_message_id.clone();
        let projection = MessageIndexProjection::new(7, reference).for_channel("group-a", "chan-a");
        store
            .put_body_and_index(&authority, b"body in the same batch", &projection)
            .unwrap();

        assert_eq!(
            store.get_body(&authority).unwrap().as_deref(),
            Some(&b"body in the same batch"[..])
        );
        assert_eq!(
            store.scan_channel("group-a", "chan-a", 10).unwrap().len(),
            1
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
