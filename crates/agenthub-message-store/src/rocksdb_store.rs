//! RocksDB-backed [`MessageBodyStore`].
//!
//! Message bodies live in a dedicated `cf_body` column family compressed by RocksDB SST block
//! compression (zstd, with a higher zstd level on the bottommost level). Bodies are keyed by
//! [`AuthorityMessageId`], so one logical message stores exactly one body regardless of fan-out.
//!
//! This module is compiled only with the `rocksdb` feature so the default build does not pull the
//! native `librocksdb-sys` dependency.

use std::path::Path;

use rocksdb::{ColumnFamilyDescriptor, DB, DBCompressionType, IteratorMode, Options, WriteBatch};

use crate::body_store::{BodyStoreError, MessageBodyStore};
use crate::ids::AuthorityMessageId;
use crate::keys::body_key;

/// Column family holding compressed message bodies.
pub const CF_BODY: &str = "cf_body";

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

    /// Open with an explicit compression type. Exposed mainly so tests can compare on-disk size
    /// against an uncompressed store.
    pub fn open_with_compression(
        path: impl AsRef<Path>,
        compression: DBCompressionType,
    ) -> Result<Self, BodyStoreError> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let cfs = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_BODY, body_cf_options(compression)),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path, cfs).map_err(backend_err)?;
        Ok(Self { db })
    }

    fn body_cf(&self) -> Result<&rocksdb::ColumnFamily, BodyStoreError> {
        self.db
            .cf_handle(CF_BODY)
            .ok_or_else(|| BodyStoreError::Backend(format!("missing column family {CF_BODY}")))
    }

    /// Flush memtables and compact `cf_body` so compression is applied to SST files. Used by tests
    /// that measure on-disk size.
    pub fn flush_and_compact(&self) -> Result<(), BodyStoreError> {
        let cf = self.body_cf()?;
        self.db.flush_cf(cf).map_err(backend_err)?;
        self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        Ok(())
    }
}

impl MessageBodyStore for RocksdbBodyStore {
    fn put_body(&mut self, id: &AuthorityMessageId, body: &[u8]) -> Result<(), BodyStoreError> {
        let cf = self.body_cf()?;
        // A single logical write uses a WriteBatch so the body put composes with future index puts.
        let mut batch = WriteBatch::default();
        batch.put_cf(cf, body_key(id), body);
        self.db.write(batch).map_err(backend_err)
    }

    fn get_body(&self, id: &AuthorityMessageId) -> Result<Option<Vec<u8>>, BodyStoreError> {
        let cf = self.body_cf()?;
        self.db.get_cf(cf, body_key(id)).map_err(backend_err)
    }

    fn contains(&self, id: &AuthorityMessageId) -> bool {
        self.body_cf()
            .ok()
            .and_then(|cf| self.db.get_cf(cf, body_key(id)).ok().flatten())
            .is_some()
    }

    fn len(&self) -> usize {
        match self.body_cf() {
            Ok(cf) => self.db.iterator_cf(cf, IteratorMode::Start).count(),
            Err(_) => 0,
        }
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

    fn dir_size(path: &Path) -> u64 {
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
                } else {
                    total += meta.len();
                }
            }
        }
        total
    }

    fn write_compact_size(compression: DBCompressionType) -> u64 {
        let dir = tempfile::tempdir().expect("tempdir");
        {
            let mut store =
                RocksdbBodyStore::open_with_compression(dir.path(), compression).expect("open");
            for (id, body) in corpus(4000) {
                store.put_body(&id, body.as_bytes()).expect("put");
            }
            store.flush_and_compact().expect("compact");
        }
        dir_size(dir.path())
    }

    #[test]
    fn body_round_trips_through_cf_body() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RocksdbBodyStore::open(dir.path()).unwrap();
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
    fn fan_out_stores_one_body_per_authority_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = RocksdbBodyStore::open(dir.path()).unwrap();
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
            "zstd on-disk size ({zstd_size}) must be smaller than uncompressed ({none_size})"
        );
    }
}
