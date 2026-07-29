//! Authority-derived index integrity and repair helpers.
//!
//! The delivery index is derived state. Callers own deriving [`MessageIndexProjection`] rows from
//! SQLite authority metadata; this module checks whether an index contains those expected refs and
//! replays them idempotently when it does not. Body checks are integrity-only: a missing `cf_body`
//! entry is reported, never rebuilt from index data.

use crate::{
    AuthorityMessageId, DeliveryMessageId, MessageBodyStore, MessageIndex, MessageIndexError,
    MessageIndexProjection, MessageRef,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageIndexNamespace {
    ById,
    Channel {
        group_id: String,
        channel_id: String,
    },
    Agent {
        agent_id: String,
    },
    Run {
        run_id: String,
    },
    Inbox {
        actor_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingIndexRef {
    pub namespace: MessageIndexNamespace,
    pub message_id: DeliveryMessageId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageIndexIntegrityReport {
    pub expected_projections: usize,
    pub missing_index_refs: Vec<MissingIndexRef>,
    pub missing_bodies: Vec<AuthorityMessageId>,
}

impl MessageIndexIntegrityReport {
    pub fn is_clean(&self) -> bool {
        self.missing_index_refs.is_empty() && self.missing_bodies.is_empty()
    }
}

pub fn check_authority_projection_integrity<I, B>(
    index: &I,
    body_store: &B,
    expected: &[MessageIndexProjection],
) -> Result<MessageIndexIntegrityReport, MessageIndexError>
where
    I: MessageIndex,
    B: MessageBodyStore,
{
    let mut report = MessageIndexIntegrityReport {
        expected_projections: expected.len(),
        ..MessageIndexIntegrityReport::default()
    };

    for projection in expected {
        let message_id = projection.reference.message_id.clone();
        if !matches_index_ref(index.get_by_id(&message_id)?, &projection.reference) {
            report.missing_index_refs.push(MissingIndexRef {
                namespace: MessageIndexNamespace::ById,
                message_id: message_id.clone(),
            });
        }

        if let Some(channel) = &projection.channel {
            let refs = index.scan_channel(&channel.group_id, &channel.channel_id, usize::MAX)?;
            if !contains_ref(&refs, &projection.reference) {
                report.missing_index_refs.push(MissingIndexRef {
                    namespace: MessageIndexNamespace::Channel {
                        group_id: channel.group_id.clone(),
                        channel_id: channel.channel_id.clone(),
                    },
                    message_id: message_id.clone(),
                });
            }
        }

        if let Some(agent_id) = &projection.agent_id {
            let refs = index.scan_agent(agent_id, usize::MAX)?;
            if !contains_ref(&refs, &projection.reference) {
                report.missing_index_refs.push(MissingIndexRef {
                    namespace: MessageIndexNamespace::Agent {
                        agent_id: agent_id.clone(),
                    },
                    message_id: message_id.clone(),
                });
            }
        }

        if let Some(run_id) = &projection.run_id {
            let refs = index.scan_run(run_id, usize::MAX)?;
            if !contains_ref(&refs, &projection.reference) {
                report.missing_index_refs.push(MissingIndexRef {
                    namespace: MessageIndexNamespace::Run {
                        run_id: run_id.clone(),
                    },
                    message_id: message_id.clone(),
                });
            }
        }

        if let Some(actor_id) = &projection.inbox_actor_id {
            let refs = index.scan_inbox(actor_id, usize::MAX)?;
            if !contains_ref(&refs, &projection.reference) {
                report.missing_index_refs.push(MissingIndexRef {
                    namespace: MessageIndexNamespace::Inbox {
                        actor_id: actor_id.clone(),
                    },
                    message_id: message_id.clone(),
                });
            }
        }

        if !body_store.contains(&projection.reference.authority_message_id) {
            report
                .missing_bodies
                .push(projection.reference.authority_message_id.clone());
        }
    }

    report.missing_bodies.sort();
    report.missing_bodies.dedup();
    Ok(report)
}

pub fn repair_authority_projection_index<I>(
    index: &I,
    expected: &[MessageIndexProjection],
) -> Result<usize, MessageIndexError>
where
    I: MessageIndex,
{
    for projection in expected {
        index.put_message(projection)?;
    }
    Ok(expected.len())
}

fn matches_index_ref(actual: Option<MessageRef>, expected: &MessageRef) -> bool {
    actual.as_ref() == Some(expected)
}

fn contains_ref(refs: &[MessageRef], expected: &MessageRef) -> bool {
    refs.iter().any(|actual| actual == expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuthorityMessageId, InMemoryBodyStore, InMemoryMessageIndex, MessageKind, MessageRef,
    };

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

    fn sample_projection(seq: u64) -> MessageIndexProjection {
        MessageIndexProjection::new(seq, sample_ref(seq))
            .for_channel("group-a", "channel-a")
            .for_agent("agent-a")
            .for_run("run-a")
            .for_inbox_actor("actor-a")
    }

    #[test]
    fn integrity_reports_missing_index_refs_and_bodies() {
        let index = InMemoryMessageIndex::new();
        let body_store = InMemoryBodyStore::new();
        let expected = vec![sample_projection(1)];

        let report = check_authority_projection_integrity(&index, &body_store, &expected).unwrap();

        assert_eq!(report.expected_projections, 1);
        assert_eq!(
            report.missing_index_refs,
            vec![
                MissingIndexRef {
                    namespace: MessageIndexNamespace::ById,
                    message_id: DeliveryMessageId::new("delivery-1"),
                },
                MissingIndexRef {
                    namespace: MessageIndexNamespace::Channel {
                        group_id: "group-a".to_string(),
                        channel_id: "channel-a".to_string(),
                    },
                    message_id: DeliveryMessageId::new("delivery-1"),
                },
                MissingIndexRef {
                    namespace: MessageIndexNamespace::Agent {
                        agent_id: "agent-a".to_string(),
                    },
                    message_id: DeliveryMessageId::new("delivery-1"),
                },
                MissingIndexRef {
                    namespace: MessageIndexNamespace::Run {
                        run_id: "run-a".to_string(),
                    },
                    message_id: DeliveryMessageId::new("delivery-1"),
                },
                MissingIndexRef {
                    namespace: MessageIndexNamespace::Inbox {
                        actor_id: "actor-a".to_string(),
                    },
                    message_id: DeliveryMessageId::new("delivery-1"),
                },
            ]
        );
        assert_eq!(
            report.missing_bodies,
            vec![AuthorityMessageId::new("auth-1")]
        );
    }

    #[test]
    fn repair_replays_index_without_rebuilding_missing_bodies() {
        let index = InMemoryMessageIndex::new();
        let body_store = InMemoryBodyStore::new();
        let expected = vec![sample_projection(1), sample_projection(2)];

        let repaired = repair_authority_projection_index(&index, &expected).unwrap();
        assert_eq!(repaired, 2);

        let report = check_authority_projection_integrity(&index, &body_store, &expected).unwrap();
        assert!(report.missing_index_refs.is_empty());
        assert_eq!(
            report.missing_bodies,
            vec![
                AuthorityMessageId::new("auth-1"),
                AuthorityMessageId::new("auth-2")
            ]
        );
    }

    #[test]
    fn integrity_is_clean_after_index_and_body_are_present() {
        let index = InMemoryMessageIndex::new();
        let body_store = InMemoryBodyStore::new();
        let expected = vec![sample_projection(1)];
        repair_authority_projection_index(&index, &expected).unwrap();
        body_store
            .put_body(&AuthorityMessageId::new("auth-1"), b"body")
            .unwrap();

        let report = check_authority_projection_integrity(&index, &body_store, &expected).unwrap();
        assert!(report.is_clean());
    }
}
