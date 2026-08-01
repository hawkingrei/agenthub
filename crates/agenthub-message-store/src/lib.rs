//! Foundation types for AgentHub message storage tiering.
//!
//! This crate is the backend-agnostic core of the storage-tiering design
//! (`docs/features/message-storage-tiering.md`): the body-free delivery index row
//! ([`MessageRef`]), the ordered-key encoding ([`keys`]), the body-store abstraction keyed by the
//! canonical logical-message identity ([`MessageBodyStore`]), and the durability outbox
//! ([`BodyOutbox`]).
//!
//! The default build has no native dependency. The RocksDB backend (where SST block compression
//! shrinks bodies at rest) implements [`MessageBodyStore`] in [`rocksdb_store`] behind the optional,
//! default-off `rocksdb` feature, so the native dependency stays isolated behind this boundary as the
//! spec requires.

pub mod body_store;
pub mod ids;
pub mod index_store;
pub mod integrity;
pub mod keys;
pub mod outbox;
pub mod reference;
#[cfg(feature = "rocksdb")]
pub mod rocksdb_store;

pub use body_store::{BodyStoreError, InMemoryBodyStore, MessageBodyStore};
pub use ids::{AuthorityMessageId, DeliveryMessageId, MessageKind};
pub use index_store::{
    AuthorityIndexProjection, InMemoryIndexReadRepairScheduler, InMemoryIndexStore, IndexFreshness,
    IndexReadRepairReason, IndexReadRepairRequest, IndexReadRepairScheduler, IndexRepairReport,
    IndexStoreError, IndexedMessageRef, MessageIndexStore, check_index_freshness,
    mark_index_repaired_through, repair_index_from_authority, repair_index_from_authority_through,
};
pub use integrity::{
    IndexAuthorityIntegrityReport, IndexAuthorityPruneReport, IndexBodyIntegrityReport,
    IntegrityCheckError, MissingBodyRef, OrphanIndexRef, check_index_refs_have_authority,
    check_index_refs_have_bodies, prune_index_refs_without_authority,
};
pub use outbox::BodyOutbox;
pub use reference::MessageRef;
#[cfg(feature = "rocksdb")]
pub use rocksdb_store::RocksdbBodyStore;

#[cfg(test)]
mod tests {
    use super::body_store::FailingBodyStore;
    use super::*;

    fn sample_ref() -> MessageRef {
        MessageRef {
            message_id: DeliveryMessageId::new("delivery-1"),
            authority_message_id: AuthorityMessageId::new("auth-1"),
            archive_document_id: Some("doc-1".to_string()),
            created_at: 1_700_000_000,
            source_kind: "team_conversation_messages".to_string(),
            message_kind: MessageKind::Text,
            correlation_id: Some("corr-1".to_string()),
            group_id: None,
            run_id: Some("run-1".to_string()),
            conversation_id: Some("conv-1".to_string()),
            agent_id: None,
        }
    }

    #[test]
    fn sort_id_encoding_preserves_bytewise_order() {
        // Within one prefix the fixed-width sort_id suffix must sort bytewise the same as numerically.
        let seqs = [0u64, 1, 2, 255, 256, 65_535, 65_536, u64::MAX - 1, u64::MAX];
        for window in seqs.windows(2) {
            let lo = keys::encode_sort_id(window[0]);
            let hi = keys::encode_sort_id(window[1]);
            assert!(
                lo < hi,
                "encoded {} should sort before {}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn channel_keys_sort_in_sequence_order_within_a_channel() {
        let mut keys_in_order: Vec<Vec<u8>> = (0..200u64)
            .map(|seq| keys::channel_key("group-a", "chan-1", seq))
            .collect();
        let expected = keys_in_order.clone();
        keys_in_order.sort();
        assert_eq!(
            keys_in_order, expected,
            "bytewise sort must match sequence order"
        );
    }

    #[test]
    fn prefixes_do_not_collide_across_namespaces_or_ids() {
        let channel = keys::channel_key("g", "c", 7);
        let agent = keys::agent_key("c", 7);
        let run = keys::run_key("c", 7);
        let inbox = keys::inbox_key("c", 7);
        let other_channel = keys::channel_key("g", "c2", 7);
        let all = [&channel, &agent, &run, &inbox, &other_channel];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "keys for different namespaces/ids must differ");
                    assert!(
                        !a.starts_with(b) && !b.starts_with(a),
                        "no key may be a prefix of another distinct namespace key"
                    );
                }
            }
        }
    }

    #[test]
    fn keys_are_deterministic_across_replay() {
        assert_eq!(
            keys::channel_key("g", "c", 42),
            keys::channel_key("g", "c", 42)
        );
        let id = AuthorityMessageId::new("auth-9");
        assert_eq!(keys::body_key(&id), keys::body_key(&id));
    }

    #[test]
    fn body_key_uses_authority_message_id() {
        let id = AuthorityMessageId::new("auth-xyz");
        let key = keys::body_key(&id);
        assert!(key.starts_with(b"body/by_message/"));
        assert!(key.ends_with(b"auth-xyz"));
    }

    #[test]
    fn high_water_key_is_separate_from_message_refs() {
        let key = keys::high_water_key("team_conversation_messages");
        assert!(key.starts_with(b"meta/high_water/"));
        assert!(!key.starts_with(b"msg/"));
        assert!(!key.starts_with(b"body/"));
    }

    #[test]
    fn message_id_key_uses_delivery_message_id() {
        let key = keys::message_id_key("delivery-xyz");
        assert!(key.starts_with(b"msg/by_id/"));
        assert!(key.ends_with(b"delivery-xyz"));
    }

    #[test]
    fn message_ref_round_trips_and_carries_no_body() {
        let original = sample_ref();
        let bytes = original.to_bytes().expect("serialize");
        let decoded = MessageRef::from_bytes(&bytes).expect("deserialize");
        assert_eq!(original, decoded);
        // A representative body string must not leak into the index row encoding.
        let encoded = String::from_utf8(bytes).unwrap();
        assert!(!encoded.contains("the full body text"));
        assert!(!encoded.contains("\"body\""));
    }

    #[test]
    fn fan_out_stores_one_body_per_authority_message() {
        // One logical message delivered to three actors -> three index rows, one body.
        let authority = AuthorityMessageId::new("auth-fanout");
        let body = b"shared meeting summary body";
        let store = InMemoryBodyStore::new();

        for actor in ["actor-a", "actor-b", "actor-c"] {
            // Each delivery row references the same authority id.
            let _row = MessageRef {
                message_id: DeliveryMessageId::new(format!("delivery-{actor}")),
                authority_message_id: authority.clone(),
                ..sample_ref()
            };
            store.put_body(&authority, body).expect("put");
        }

        assert_eq!(store.len(), 1, "fan-out must not duplicate the body");
        assert_eq!(
            store.get_body(&authority).unwrap().as_deref(),
            Some(&body[..])
        );
    }

    #[test]
    fn outbox_stages_inside_authority_then_drains_to_store() {
        let id = AuthorityMessageId::new("auth-drain");
        let body = b"durable body";
        let mut outbox = BodyOutbox::new();
        let store = InMemoryBodyStore::new();

        outbox.stage(&id, body);
        assert_eq!(outbox.pending_len(), 1);
        assert!(!store.contains(&id));

        let confirmed = outbox.drain_into(&store);
        assert_eq!(confirmed, 1);
        assert!(outbox.is_empty(), "confirmed bodies leave the outbox");
        assert_eq!(store.get_body(&id).unwrap().as_deref(), Some(&body[..]));
    }

    #[test]
    fn body_survives_store_failure_and_is_recovered_by_replay() {
        // Simulate a cf_body write failure after the authority commit: the body stays in the outbox
        // and a later drain recovers it. No body is lost.
        let id = AuthorityMessageId::new("auth-replay");
        let body = b"body that must not be lost";
        let mut outbox = BodyOutbox::new();
        let store = FailingBodyStore::new(1); // first write fails

        outbox.stage(&id, body);
        let confirmed = outbox.drain_into(&store);
        assert_eq!(confirmed, 0, "the failing write confirms nothing");
        assert!(
            outbox.contains(&id),
            "the body stays pending after a failed write"
        );
        assert_eq!(store.len(), 0);

        // Retry (e.g. background drainer after a crash) succeeds.
        let confirmed = outbox.drain_into(&store);
        assert_eq!(confirmed, 1);
        assert!(outbox.is_empty());
        assert_eq!(store.get_body(&id).unwrap().as_deref(), Some(&body[..]));
    }

    #[test]
    fn authority_repair_rebuilds_body_free_index_refs() {
        let index = InMemoryIndexStore::new();
        let authority = AuthorityMessageId::new("auth-repair");
        let first = MessageRef {
            message_id: DeliveryMessageId::new("delivery-1"),
            authority_message_id: authority.clone(),
            ..sample_ref()
        };
        let second = MessageRef {
            message_id: DeliveryMessageId::new("delivery-2"),
            authority_message_id: authority,
            ..sample_ref()
        };

        let projections = vec![
            AuthorityIndexProjection {
                key: keys::channel_key("group-a", "chan-1", 1),
                message_ref: first.clone(),
            },
            AuthorityIndexProjection {
                key: keys::channel_key("group-a", "chan-1", 2),
                message_ref: second.clone(),
            },
        ];

        let report = repair_index_from_authority(&index, projections).expect("repair");
        assert_eq!(report.repaired_refs, 2);
        assert_eq!(
            index
                .scan_prefix(&keys::channel_prefix("group-a", "chan-1"))
                .expect("scan"),
            vec![first, second]
        );
    }

    #[test]
    fn high_water_guard_reports_lag_until_repair_marks_authority_max() {
        let index = InMemoryIndexStore::new();
        assert_eq!(
            check_index_freshness(&index, "team_conversation_messages", 9).expect("check"),
            IndexFreshness::Lagging {
                indexed_through: None,
                authority_max: 9
            }
        );

        mark_index_repaired_through(&index, "team_conversation_messages", 7).expect("mark");
        assert_eq!(
            check_index_freshness(&index, "team_conversation_messages", 9).expect("check"),
            IndexFreshness::Lagging {
                indexed_through: Some(7),
                authority_max: 9
            }
        );

        mark_index_repaired_through(&index, "team_conversation_messages", 9).expect("mark");
        assert_eq!(
            check_index_freshness(&index, "team_conversation_messages", 9).expect("check"),
            IndexFreshness::Fresh { indexed_through: 9 }
        );

        mark_index_repaired_through(&index, "team_conversation_messages", 8).expect("mark");
        assert_eq!(
            check_index_freshness(&index, "team_conversation_messages", 9).expect("check"),
            IndexFreshness::Fresh { indexed_through: 9 },
            "older repair passes must not lower the high-water mark"
        );
    }

    #[test]
    fn repair_through_marks_high_water_after_refs_are_rebuilt() {
        let index = InMemoryIndexStore::new();
        let message_ref = sample_ref();
        let projection = AuthorityIndexProjection {
            key: keys::run_key("run-1", 12),
            message_ref: message_ref.clone(),
        };

        let report =
            repair_index_from_authority_through(&index, [projection], "team_actor_messages", 12)
                .expect("repair");

        assert_eq!(report.repaired_refs, 1);
        assert_eq!(
            index.scan_prefix(&keys::run_prefix("run-1")).expect("scan"),
            vec![message_ref]
        );
        assert_eq!(
            check_index_freshness(&index, "team_actor_messages", 12).expect("check"),
            IndexFreshness::Fresh {
                indexed_through: 12
            }
        );
    }

    #[test]
    fn read_repair_scheduler_keeps_highest_requested_authority_bound() {
        let scheduler = InMemoryIndexReadRepairScheduler::new();
        scheduler
            .schedule_read_repair(IndexReadRepairRequest {
                stream_id: "team_actor_messages".to_string(),
                authority_max: 12,
                reason: IndexReadRepairReason::Lagging {
                    indexed_through: Some(7),
                },
            })
            .expect("schedule initial repair");
        scheduler
            .schedule_read_repair(IndexReadRepairRequest {
                stream_id: "team_actor_messages".to_string(),
                authority_max: 9,
                reason: IndexReadRepairReason::Lagging {
                    indexed_through: Some(8),
                },
            })
            .expect("schedule older repair");
        scheduler
            .schedule_read_repair(IndexReadRepairRequest {
                stream_id: "team_run_events".to_string(),
                authority_max: 3,
                reason: IndexReadRepairReason::Lagging {
                    indexed_through: None,
                },
            })
            .expect("schedule missing high-water repair");

        let repairs = scheduler.pending_repairs();
        assert_eq!(repairs.len(), 2);
        assert!(repairs.contains(&IndexReadRepairRequest {
            stream_id: "team_actor_messages".to_string(),
            authority_max: 12,
            reason: IndexReadRepairReason::Lagging {
                indexed_through: Some(7),
            },
        }));
        assert!(repairs.contains(&IndexReadRepairRequest {
            stream_id: "team_run_events".to_string(),
            authority_max: 3,
            reason: IndexReadRepairReason::Lagging {
                indexed_through: None,
            },
        }));
    }

    #[test]
    fn integrity_check_reports_index_refs_missing_bodies() {
        let index = InMemoryIndexStore::new();
        let bodies = InMemoryBodyStore::new();
        let present_id = AuthorityMessageId::new("auth-present");
        let missing_id = AuthorityMessageId::new("auth-missing");

        bodies
            .put_body(&present_id, b"present body")
            .expect("put present body");

        let present_ref = MessageRef {
            message_id: DeliveryMessageId::new("delivery-present"),
            authority_message_id: present_id,
            ..sample_ref()
        };
        let first_missing_ref = MessageRef {
            message_id: DeliveryMessageId::new("delivery-missing-1"),
            authority_message_id: missing_id.clone(),
            source_kind: "team_actor_messages".to_string(),
            ..sample_ref()
        };
        let second_missing_ref = MessageRef {
            message_id: DeliveryMessageId::new("delivery-missing-2"),
            authority_message_id: missing_id.clone(),
            source_kind: "team_actor_messages".to_string(),
            ..sample_ref()
        };

        index
            .put_ref(&keys::channel_key("group-a", "chan-1", 1), &present_ref)
            .expect("put present ref");
        index
            .put_ref(
                &keys::channel_key("group-a", "chan-1", 2),
                &first_missing_ref,
            )
            .expect("put first missing ref");
        index
            .put_ref(
                &keys::channel_key("group-a", "chan-1", 3),
                &second_missing_ref,
            )
            .expect("put second missing ref");

        let report = check_index_refs_have_bodies(
            &index,
            &bodies,
            [keys::channel_prefix("group-a", "chan-1")],
        )
        .expect("integrity check");

        assert_eq!(report.scanned_refs, 3);
        assert!(!report.is_clean());
        assert_eq!(report.missing_body_refs.len(), 2);
        assert_eq!(
            report
                .missing_body_refs
                .iter()
                .map(|missing| missing.message_id.as_str())
                .collect::<Vec<_>>(),
            vec!["delivery-missing-1", "delivery-missing-2"]
        );
        assert_eq!(report.missing_authority_message_ids, vec![missing_id]);
    }

    #[test]
    fn integrity_check_reports_index_refs_missing_authority() {
        let index = InMemoryIndexStore::new();
        let authority_id = AuthorityMessageId::new("auth-present");
        let orphan_id = AuthorityMessageId::new("auth-orphan");

        let authority_ref = MessageRef {
            message_id: DeliveryMessageId::new("delivery-present"),
            authority_message_id: authority_id.clone(),
            ..sample_ref()
        };
        let first_orphan_ref = MessageRef {
            message_id: DeliveryMessageId::new("delivery-orphan-1"),
            authority_message_id: orphan_id.clone(),
            source_kind: "team_actor_messages".to_string(),
            ..sample_ref()
        };
        let second_orphan_ref = MessageRef {
            message_id: DeliveryMessageId::new("delivery-orphan-2"),
            authority_message_id: orphan_id.clone(),
            source_kind: "team_actor_messages".to_string(),
            ..sample_ref()
        };

        index
            .put_ref(&keys::run_key("run-1", 1), &authority_ref)
            .expect("put authority ref");
        index
            .put_ref(&keys::run_key("run-1", 2), &first_orphan_ref)
            .expect("put first orphan ref");
        index
            .put_ref(&keys::run_key("run-1", 3), &second_orphan_ref)
            .expect("put second orphan ref");

        let report =
            check_index_refs_have_authority(&index, [keys::run_prefix("run-1")], [authority_id])
                .expect("orphan check");

        assert_eq!(report.scanned_refs, 3);
        assert!(!report.is_clean());
        assert_eq!(
            report
                .orphan_refs
                .iter()
                .map(|orphan| orphan.message_id.as_str())
                .collect::<Vec<_>>(),
            vec!["delivery-orphan-1", "delivery-orphan-2"]
        );
        assert_eq!(report.orphan_authority_message_ids, vec![orphan_id]);
    }

    #[test]
    fn explicit_orphan_prune_deletes_only_refs_without_authority() {
        let index = InMemoryIndexStore::new();
        let authority_id = AuthorityMessageId::new("auth-present");
        let orphan_id = AuthorityMessageId::new("auth-orphan");

        let authority_ref = MessageRef {
            message_id: DeliveryMessageId::new("delivery-present"),
            authority_message_id: authority_id.clone(),
            ..sample_ref()
        };
        let orphan_ref = MessageRef {
            message_id: DeliveryMessageId::new("delivery-orphan"),
            authority_message_id: orphan_id.clone(),
            source_kind: "team_run_events".to_string(),
            ..sample_ref()
        };
        let authority_key = keys::run_key("run-1", 1);
        let orphan_key = keys::run_key("run-1", 2);

        index
            .put_ref(&authority_key, &authority_ref)
            .expect("put authority ref");
        index
            .put_ref(&orphan_key, &orphan_ref)
            .expect("put orphan ref");
        mark_index_repaired_through(&index, "team_run_events", 2).expect("mark high-water");

        let check_report = check_index_refs_have_authority(
            &index,
            [keys::run_prefix("run-1")],
            [authority_id.clone()],
        )
        .expect("orphan check");
        assert_eq!(check_report.orphan_refs.len(), 1);
        assert_eq!(
            index.get_ref(&orphan_key).expect("get orphan before prune"),
            Some(orphan_ref.clone()),
            "diagnostic check must not delete index refs"
        );

        let prune_report =
            prune_index_refs_without_authority(&index, [keys::run_prefix("run-1")], [authority_id])
                .expect("orphan prune");

        assert_eq!(prune_report.scanned_refs, 2);
        assert_eq!(prune_report.pruned_refs.len(), 1);
        assert_eq!(prune_report.pruned_refs[0].index_key, orphan_key);
        assert_eq!(prune_report.pruned_authority_message_ids, vec![orphan_id]);
        assert_eq!(
            index.get_ref(&authority_key).expect("get authority"),
            Some(authority_ref)
        );
        assert_eq!(index.get_ref(&orphan_key).expect("get orphan"), None);
        assert_eq!(
            check_index_freshness(&index, "team_run_events", 2).expect("check high-water"),
            IndexFreshness::Fresh { indexed_through: 2 },
            "prune must not lower or rewrite high-water markers"
        );
    }

    // --- Compression validation -------------------------------------------------------------------
    //
    // The storage-tiering pivot (design B) puts bodies in RocksDB so SST *block* compression shrinks
    // them. The key claim is that aggregating many short chat messages into a block compresses far
    // better than compressing each short message on its own. These tests validate that premise on a
    // representative chat corpus using the same zstd algorithm RocksDB would use, so the expected
    // win is measured rather than assumed.

    fn chat_corpus() -> Vec<String> {
        // Realistic, repetitive chat-like traffic: short human/agent lines and small tool-call JSON.
        let speakers = ["alice", "bob", "agent-claude", "agent-codex"];
        let phrases = [
            "Can you take a look at the failing test in the storage module?",
            "Sure, I'll check the dual-write path and report back.",
            "The transcript looks good, shipping the change now.",
            "Please rebase onto main and re-run the bazel crate tests.",
            "Done. CI is green and the PR is ready for review.",
        ];
        let mut out = Vec::new();
        for i in 0..60u32 {
            let speaker = speakers[(i as usize) % speakers.len()];
            let phrase = phrases[(i as usize) % phrases.len()];
            out.push(format!(
                "{{\"speaker\":\"{speaker}\",\"seq\":{i},\"text\":\"{phrase}\"}}"
            ));
        }
        out
    }

    fn zstd_len(bytes: &[u8]) -> usize {
        zstd::bulk::compress(bytes, 3).expect("zstd compress").len()
    }

    #[test]
    fn block_aggregated_compression_beats_per_message_and_raw() {
        let corpus = chat_corpus();
        let raw_total: usize = corpus.iter().map(|m| m.len()).sum();

        // Per-message: compress each short message independently (no cross-message context).
        let per_message_total: usize = corpus.iter().map(|m| zstd_len(m.as_bytes())).sum();

        // Block-aggregated: concatenate then compress once, the way an SST data block shares context
        // across many small records.
        let aggregated = corpus.join("\n");
        let aggregated_total = zstd_len(aggregated.as_bytes());

        // Block aggregation must beat per-message compression, and must shrink the raw bytes.
        assert!(
            aggregated_total < per_message_total,
            "block-aggregated ({aggregated_total}) must beat per-message ({per_message_total})"
        );
        assert!(
            aggregated_total < raw_total,
            "block-aggregated ({aggregated_total}) must shrink raw ({raw_total})"
        );

        // Conservative floor on the realized saving for this repetitive chat corpus.
        let ratio = aggregated_total as f64 / raw_total as f64;
        assert!(ratio < 0.6, "expected >40% saving, got ratio {ratio:.3}");
    }
}
