use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ACTOR_MAIN_PEER_ID: &str = "main";
pub const ACTOR_NODE_PEER_ID: &str = "node";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorIdentityKind {
    Agent,
    Human,
}

pub fn infer_actor_identity_kind(actor_id: &str) -> ActorIdentityKind {
    let normalized = actor_id.trim().to_ascii_lowercase();
    if normalized == "user"
        || normalized == "human"
        || normalized.starts_with("user:")
        || normalized.starts_with("human:")
    {
        return ActorIdentityKind::Human;
    }
    ActorIdentityKind::Agent
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorMessageTransport {
    Local,
    Remote,
}

impl ActorMessageTransport {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorMessageStatus {
    Pending,
    Delivered,
    DeadLetter,
}

impl ActorMessageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Delivered => "delivered",
            Self::DeadLetter => "dead_letter",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActorMessageKind {
    HumanRequest,
    #[default]
    CoordinationRequest,
    TriggerEvent,
    TaskSignal,
    ThreadReply,
    SystemNotice,
}

impl ActorMessageKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HumanRequest => "human_request",
            Self::CoordinationRequest => "coordination_request",
            Self::TriggerEvent => "trigger_event",
            Self::TaskSignal => "task_signal",
            Self::ThreadReply => "thread_reply",
            Self::SystemNotice => "system_notice",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActorMessageHandlingDisposition {
    #[default]
    Untriaged,
    Ignored,
    Watching,
    Claimed,
    Completed,
    Released,
}

impl ActorMessageHandlingDisposition {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Untriaged => "untriaged",
            Self::Ignored => "ignored",
            Self::Watching => "watching",
            Self::Claimed => "claimed",
            Self::Completed => "completed",
            Self::Released => "released",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorThreadClaimStatus {
    Claimed,
    Released,
    Completed,
}

impl ActorThreadClaimStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::Released => "released",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorMessageTaskRelation {
    SpawnedTask,
    RelatedTask,
    EvidenceForTask,
}

impl ActorMessageTaskRelation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SpawnedTask => "spawned_task",
            Self::RelatedTask => "related_task",
            Self::EvidenceForTask => "evidence_for_task",
        }
    }
}

pub fn parse_actor_message_kind(raw: &str) -> ActorMessageKind {
    match raw.trim() {
        "human_request" => ActorMessageKind::HumanRequest,
        "trigger_event" => ActorMessageKind::TriggerEvent,
        "task_signal" => ActorMessageKind::TaskSignal,
        "thread_reply" => ActorMessageKind::ThreadReply,
        "system_notice" => ActorMessageKind::SystemNotice,
        _ => ActorMessageKind::CoordinationRequest,
    }
}

pub fn parse_actor_message_handling_disposition(raw: &str) -> ActorMessageHandlingDisposition {
    match raw.trim() {
        "ignored" => ActorMessageHandlingDisposition::Ignored,
        "watching" => ActorMessageHandlingDisposition::Watching,
        "claimed" => ActorMessageHandlingDisposition::Claimed,
        "completed" => ActorMessageHandlingDisposition::Completed,
        "released" => ActorMessageHandlingDisposition::Released,
        _ => ActorMessageHandlingDisposition::Untriaged,
    }
}

pub fn parse_actor_thread_claim_status(raw: &str) -> Option<ActorThreadClaimStatus> {
    match raw.trim() {
        "claimed" => Some(ActorThreadClaimStatus::Claimed),
        "released" => Some(ActorThreadClaimStatus::Released),
        "completed" => Some(ActorThreadClaimStatus::Completed),
        _ => None,
    }
}

pub fn parse_actor_message_task_relation(raw: &str) -> Option<ActorMessageTaskRelation> {
    match raw.trim() {
        "spawned_task" => Some(ActorMessageTaskRelation::SpawnedTask),
        "related_task" => Some(ActorMessageTaskRelation::RelatedTask),
        "evidence_for_task" => Some(ActorMessageTaskRelation::EvidenceForTask),
        _ => None,
    }
}

fn payload_positive_i64(payload: &Value, key: &str) -> Option<i64> {
    payload
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
}

fn payload_trimmed_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorMessageTopicMetadata {
    pub topic_key: String,
    pub task_id: Option<String>,
    pub root_message_id: Option<i64>,
}

pub fn actor_message_payload_task_id(payload: &Value) -> Option<String> {
    payload_trimmed_string(payload, "task_id")
}

pub fn actor_message_payload_thread_root_message_id(payload: &Value) -> Option<i64> {
    payload_positive_i64(payload, "thread_root_message_id")
}

pub fn derive_actor_message_topic_metadata(
    message_id: i64,
    payload: &Value,
    explicit_task_id: Option<&str>,
) -> Option<ActorMessageTopicMetadata> {
    let task_id = explicit_task_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| actor_message_payload_task_id(payload));
    let root_message_id = actor_message_payload_thread_root_message_id(payload);
    let task_message_id = payload_positive_i64(payload, "task_message_id");
    let correlation_id = payload_trimmed_string(payload, "correlation_id");

    if let (Some(task_id), Some(root_message_id)) = (task_id.clone(), root_message_id) {
        return Some(ActorMessageTopicMetadata {
            topic_key: format!("task:{task_id}:thread:{root_message_id}"),
            task_id: Some(task_id),
            root_message_id: Some(root_message_id),
        });
    }
    if let (Some(task_id), Some(task_message_id)) = (task_id.clone(), task_message_id) {
        return Some(ActorMessageTopicMetadata {
            topic_key: format!("task:{task_id}:message:{task_message_id}"),
            task_id: Some(task_id),
            root_message_id: None,
        });
    }
    if let Some(correlation_id) = correlation_id {
        return Some(ActorMessageTopicMetadata {
            topic_key: format!("correlation:{correlation_id}"),
            task_id,
            root_message_id,
        });
    }
    task_id.map(|task_id| ActorMessageTopicMetadata {
        topic_key: format!("task:{task_id}:mailbox:{message_id}"),
        task_id: Some(task_id),
        root_message_id,
    })
}

pub fn infer_actor_message_kind(
    from_actor_id: &str,
    payload: &Value,
    explicit_kind: Option<ActorMessageKind>,
) -> ActorMessageKind {
    if let Some(explicit_kind) = explicit_kind {
        return explicit_kind;
    }
    if payload
        .as_object()
        .and_then(|map| map.get("thread_root_message_id"))
        .and_then(Value::as_i64)
        .is_some_and(|value| value > 0)
    {
        return ActorMessageKind::ThreadReply;
    }
    if let Some(payload_type) = payload
        .as_object()
        .and_then(|map| map.get("type"))
        .and_then(Value::as_str)
        .map(str::trim)
    {
        match payload_type {
            "trigger_event" => return ActorMessageKind::TriggerEvent,
            "task_signal" => return ActorMessageKind::TaskSignal,
            "thread_reply" => return ActorMessageKind::ThreadReply,
            "system_notice" | "permission_review_request" => {
                return ActorMessageKind::SystemNotice;
            }
            _ => {}
        }
    }
    if infer_actor_identity_kind(from_actor_id) == ActorIdentityKind::Human {
        return ActorMessageKind::HumanRequest;
    }
    ActorMessageKind::CoordinationRequest
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorMessageRecord {
    pub message_id: i64,
    pub run_id: String,
    pub from_actor_id: String,
    pub from_peer_id: String,
    pub from_actor_kind: ActorIdentityKind,
    pub to_actor_id: String,
    pub to_peer_id: String,
    pub to_actor_kind: ActorIdentityKind,
    pub channel: String,
    pub transport: ActorMessageTransport,
    pub route: Option<Value>,
    pub payload: Value,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub message_kind: ActorMessageKind,
    pub status: ActorMessageStatus,
    #[serde(default)]
    pub handling_disposition: ActorMessageHandlingDisposition,
    pub handled_by_actor_id: Option<String>,
    pub thread_topic_key: Option<String>,
    pub thread_claim_status: Option<ActorThreadClaimStatus>,
    pub thread_owner_actor_id: Option<String>,
    pub thread_lease_expires_at: Option<i64>,
    pub linked_task_id: Option<String>,
    pub linked_task_relation: Option<ActorMessageTaskRelation>,
    pub created_at: i64,
    pub delivered_at: Option<i64>,
    pub handled_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::{
        ActorIdentityKind, ActorMessageHandlingDisposition, ActorMessageKind,
        ActorMessageTaskRelation, ActorThreadClaimStatus, derive_actor_message_topic_metadata,
        infer_actor_identity_kind, infer_actor_message_kind,
        parse_actor_message_handling_disposition, parse_actor_message_kind,
        parse_actor_message_task_relation, parse_actor_thread_claim_status,
    };
    use serde_json::json;

    #[test]
    fn infer_actor_identity_kind_marks_human_aliases() {
        assert_eq!(infer_actor_identity_kind("user"), ActorIdentityKind::Human);
        assert_eq!(infer_actor_identity_kind("human"), ActorIdentityKind::Human);
        assert_eq!(
            infer_actor_identity_kind("user:alice"),
            ActorIdentityKind::Human
        );
        assert_eq!(
            infer_actor_identity_kind("HUMAN:operator"),
            ActorIdentityKind::Human
        );
        assert_eq!(
            infer_actor_identity_kind("agent-worker-1"),
            ActorIdentityKind::Agent
        );
    }

    #[test]
    fn derive_actor_message_topic_metadata_prefers_task_thread_scope() {
        let metadata = derive_actor_message_topic_metadata(
            7,
            &json!({
                "task_id": "task-1",
                "thread_root_message_id": 42,
                "task_message_id": 99,
                "correlation_id": "corr-1"
            }),
            None,
        )
        .expect("derive metadata");
        assert_eq!(metadata.topic_key, "task:task-1:thread:42");
        assert_eq!(metadata.task_id.as_deref(), Some("task-1"));
        assert_eq!(metadata.root_message_id, Some(42));
    }

    #[test]
    fn derive_actor_message_topic_metadata_uses_explicit_task_id_for_mailbox_scope() {
        let metadata =
            derive_actor_message_topic_metadata(9, &json!({"text":"hello"}), Some("task-9"))
                .expect("derive metadata");
        assert_eq!(metadata.topic_key, "task:task-9:mailbox:9");
        assert_eq!(metadata.task_id.as_deref(), Some("task-9"));
        assert_eq!(metadata.root_message_id, None);
    }

    #[test]
    fn derive_actor_message_topic_metadata_uses_task_message_before_correlation() {
        let metadata = derive_actor_message_topic_metadata(
            11,
            &json!({
                "task_id": "task-11",
                "task_message_id": 88,
                "correlation_id": "corr-11"
            }),
            None,
        )
        .expect("derive metadata");
        assert_eq!(metadata.topic_key, "task:task-11:message:88");
        assert_eq!(metadata.task_id.as_deref(), Some("task-11"));
        assert_eq!(metadata.root_message_id, None);
    }

    #[test]
    fn derive_actor_message_topic_metadata_falls_back_to_correlation_scope() {
        let metadata = derive_actor_message_topic_metadata(
            12,
            &json!({
                "correlation_id": "corr-12",
                "task_id": "task-12"
            }),
            None,
        )
        .expect("derive metadata");
        assert_eq!(metadata.topic_key, "correlation:corr-12");
        assert_eq!(metadata.task_id.as_deref(), Some("task-12"));
        assert_eq!(metadata.root_message_id, None);
    }

    #[test]
    fn derive_actor_message_topic_metadata_returns_none_without_scope() {
        assert_eq!(
            derive_actor_message_topic_metadata(13, &json!({"task_id":"   "}), Some("   ")),
            None
        );
        assert_eq!(
            derive_actor_message_topic_metadata(
                14,
                &json!({"thread_root_message_id": 0, "task_message_id": -1}),
                None
            ),
            None
        );
    }

    #[test]
    fn parse_actor_message_helpers_cover_all_known_values() {
        assert_eq!(
            parse_actor_message_kind("human_request"),
            ActorMessageKind::HumanRequest
        );
        assert_eq!(
            parse_actor_message_kind("trigger_event"),
            ActorMessageKind::TriggerEvent
        );
        assert_eq!(
            parse_actor_message_kind("task_signal"),
            ActorMessageKind::TaskSignal
        );
        assert_eq!(
            parse_actor_message_kind("thread_reply"),
            ActorMessageKind::ThreadReply
        );
        assert_eq!(
            parse_actor_message_kind("system_notice"),
            ActorMessageKind::SystemNotice
        );
        assert_eq!(
            parse_actor_message_kind("unknown"),
            ActorMessageKind::CoordinationRequest
        );

        assert_eq!(
            parse_actor_message_handling_disposition("ignored"),
            ActorMessageHandlingDisposition::Ignored
        );
        assert_eq!(
            parse_actor_message_handling_disposition("watching"),
            ActorMessageHandlingDisposition::Watching
        );
        assert_eq!(
            parse_actor_message_handling_disposition("claimed"),
            ActorMessageHandlingDisposition::Claimed
        );
        assert_eq!(
            parse_actor_message_handling_disposition("completed"),
            ActorMessageHandlingDisposition::Completed
        );
        assert_eq!(
            parse_actor_message_handling_disposition("released"),
            ActorMessageHandlingDisposition::Released
        );
        assert_eq!(
            parse_actor_message_handling_disposition("unknown"),
            ActorMessageHandlingDisposition::Untriaged
        );

        assert_eq!(
            parse_actor_thread_claim_status("claimed"),
            Some(ActorThreadClaimStatus::Claimed)
        );
        assert_eq!(
            parse_actor_thread_claim_status("released"),
            Some(ActorThreadClaimStatus::Released)
        );
        assert_eq!(
            parse_actor_thread_claim_status("completed"),
            Some(ActorThreadClaimStatus::Completed)
        );
        assert_eq!(parse_actor_thread_claim_status("unknown"), None);

        assert_eq!(
            parse_actor_message_task_relation("spawned_task"),
            Some(ActorMessageTaskRelation::SpawnedTask)
        );
        assert_eq!(
            parse_actor_message_task_relation("related_task"),
            Some(ActorMessageTaskRelation::RelatedTask)
        );
        assert_eq!(
            parse_actor_message_task_relation("evidence_for_task"),
            Some(ActorMessageTaskRelation::EvidenceForTask)
        );
        assert_eq!(parse_actor_message_task_relation("unknown"), None);
    }

    #[test]
    fn infer_actor_message_kind_prefers_explicit_then_payload_then_actor_identity() {
        assert_eq!(
            infer_actor_message_kind(
                "agent-worker-1",
                &json!({"type":"task_signal"}),
                Some(ActorMessageKind::SystemNotice),
            ),
            ActorMessageKind::SystemNotice
        );
        assert_eq!(
            infer_actor_message_kind(
                "agent-worker-1",
                &json!({"thread_root_message_id": 21, "type":"trigger_event"}),
                None,
            ),
            ActorMessageKind::ThreadReply
        );
        assert_eq!(
            infer_actor_message_kind(
                "agent-worker-1",
                &json!({"type":"permission_review_request"}),
                None,
            ),
            ActorMessageKind::SystemNotice
        );
        assert_eq!(
            infer_actor_message_kind("user:alice", &json!({"type":"unknown"}), None),
            ActorMessageKind::HumanRequest
        );
        assert_eq!(
            infer_actor_message_kind("agent-worker-1", &json!({"type":"unknown"}), None),
            ActorMessageKind::CoordinationRequest
        );
    }
}
