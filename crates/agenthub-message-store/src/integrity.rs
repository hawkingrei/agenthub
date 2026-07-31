//! Integrity checks across rebuildable delivery refs and durable message bodies.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::body_store::MessageBodyStore;
use crate::ids::{AuthorityMessageId, DeliveryMessageId};
use crate::index_store::{IndexStoreError, IndexedMessageRef, MessageIndexStore};

#[derive(Debug, Error)]
pub enum IntegrityCheckError {
    #[error("message index integrity scan failed: {0}")]
    Index(#[from] IndexStoreError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingBodyRef {
    pub message_id: DeliveryMessageId,
    pub authority_message_id: AuthorityMessageId,
    pub source_kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrphanIndexRef {
    pub index_key: Vec<u8>,
    pub message_id: DeliveryMessageId,
    pub authority_message_id: AuthorityMessageId,
    pub source_kind: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexBodyIntegrityReport {
    pub scanned_refs: usize,
    pub missing_body_refs: Vec<MissingBodyRef>,
    pub missing_authority_message_ids: Vec<AuthorityMessageId>,
}

impl IndexBodyIntegrityReport {
    pub fn is_clean(&self) -> bool {
        self.missing_body_refs.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexAuthorityIntegrityReport {
    pub scanned_refs: usize,
    pub orphan_refs: Vec<OrphanIndexRef>,
    pub orphan_authority_message_ids: Vec<AuthorityMessageId>,
}

impl IndexAuthorityIntegrityReport {
    pub fn is_clean(&self) -> bool {
        self.orphan_refs.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexAuthorityPruneReport {
    pub scanned_refs: usize,
    pub pruned_refs: Vec<OrphanIndexRef>,
    pub pruned_authority_message_ids: Vec<AuthorityMessageId>,
}

impl IndexAuthorityPruneReport {
    pub fn is_clean(&self) -> bool {
        self.pruned_refs.is_empty()
    }
}

/// Verify that every index ref under the provided prefixes points at an existing body.
///
/// This check is diagnostic only. It does not rebuild or prune `cf_index`, and it does not use the
/// archive as an authority source.
pub fn check_index_refs_have_bodies<I, B, P>(
    index: &I,
    bodies: &B,
    prefixes: impl IntoIterator<Item = P>,
) -> Result<IndexBodyIntegrityReport, IntegrityCheckError>
where
    I: MessageIndexStore + ?Sized,
    B: MessageBodyStore + ?Sized,
    P: AsRef<[u8]>,
{
    let mut report = IndexBodyIntegrityReport::default();
    let mut missing_ids = BTreeSet::new();

    for prefix in prefixes {
        for message_ref in index.scan_prefix(prefix.as_ref())? {
            report.scanned_refs += 1;
            if bodies.contains(&message_ref.authority_message_id) {
                continue;
            }

            missing_ids.insert(message_ref.authority_message_id.clone());
            report.missing_body_refs.push(MissingBodyRef {
                message_id: message_ref.message_id,
                authority_message_id: message_ref.authority_message_id,
                source_kind: message_ref.source_kind,
            });
        }
    }

    report.missing_authority_message_ids = missing_ids.into_iter().collect();
    Ok(report)
}

/// Verify that every index ref under the provided prefixes is backed by caller-supplied authority.
///
/// The caller owns the authoritative SQLite query and provides the expected logical-message ids. This
/// helper intentionally does not query SQLite by itself, so the message-store crate stays independent
/// from individual authority schemas.
pub fn check_index_refs_have_authority<I, P, A>(
    index: &I,
    prefixes: impl IntoIterator<Item = P>,
    authority_message_ids: impl IntoIterator<Item = A>,
) -> Result<IndexAuthorityIntegrityReport, IntegrityCheckError>
where
    I: MessageIndexStore + ?Sized,
    P: AsRef<[u8]>,
    A: Into<AuthorityMessageId>,
{
    let authority_ids = authority_message_ids
        .into_iter()
        .map(Into::into)
        .collect::<BTreeSet<_>>();
    let mut report = IndexAuthorityIntegrityReport::default();
    let mut orphan_ids = BTreeSet::new();

    for prefix in prefixes {
        for indexed_ref in index.scan_prefix_entries(prefix.as_ref())? {
            report.scanned_refs += 1;
            if authority_ids.contains(&indexed_ref.message_ref.authority_message_id) {
                continue;
            }
            push_orphan_ref(&mut report.orphan_refs, &mut orphan_ids, indexed_ref);
        }
    }

    report.orphan_authority_message_ids = orphan_ids.into_iter().collect();
    Ok(report)
}

/// Delete index refs under the provided prefixes that are absent from caller-supplied authority.
///
/// This is the explicit prune mode for the rebuildable delivery index. It never consults `cf_body`
/// or archive rows, and it never lowers high-water marks. Callers must provide the SQLite authority
/// id set for the exact namespace they are pruning.
pub fn prune_index_refs_without_authority<I, P, A>(
    index: &I,
    prefixes: impl IntoIterator<Item = P>,
    authority_message_ids: impl IntoIterator<Item = A>,
) -> Result<IndexAuthorityPruneReport, IntegrityCheckError>
where
    I: MessageIndexStore + ?Sized,
    P: AsRef<[u8]>,
    A: Into<AuthorityMessageId>,
{
    let authority_ids = authority_message_ids
        .into_iter()
        .map(Into::into)
        .collect::<BTreeSet<_>>();
    let mut report = IndexAuthorityPruneReport::default();
    let mut pruned_ids = BTreeSet::new();

    for prefix in prefixes {
        for indexed_ref in index.scan_prefix_entries(prefix.as_ref())? {
            report.scanned_refs += 1;
            if authority_ids.contains(&indexed_ref.message_ref.authority_message_id) {
                continue;
            }
            index.delete_ref(&indexed_ref.key)?;
            push_orphan_ref(&mut report.pruned_refs, &mut pruned_ids, indexed_ref);
        }
    }

    report.pruned_authority_message_ids = pruned_ids.into_iter().collect();
    Ok(report)
}

fn push_orphan_ref(
    refs: &mut Vec<OrphanIndexRef>,
    orphan_ids: &mut BTreeSet<AuthorityMessageId>,
    indexed_ref: IndexedMessageRef,
) {
    let message_ref = indexed_ref.message_ref;
    orphan_ids.insert(message_ref.authority_message_id.clone());
    refs.push(OrphanIndexRef {
        index_key: indexed_ref.key,
        message_id: message_ref.message_id,
        authority_message_id: message_ref.authority_message_id,
        source_kind: message_ref.source_kind,
    });
}
