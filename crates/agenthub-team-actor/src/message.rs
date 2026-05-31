use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const ACTOR_MAIN_PEER_ID: &str = "main";
pub const ACTOR_NODE_PEER_ID: &str = "node";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorIdentityKind {
    Agent,
    Human,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorMessageSourceKind {
    Human,
    Agent,
    Trigger,
    System,
}

impl ActorMessageSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Trigger => "trigger",
            Self::System => "system",
        }
    }
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

pub fn parse_actor_message_source_kind(raw: &str) -> Option<ActorMessageSourceKind> {
    match raw.trim() {
        "human" => Some(ActorMessageSourceKind::Human),
        "agent" => Some(ActorMessageSourceKind::Agent),
        "trigger" => Some(ActorMessageSourceKind::Trigger),
        "system" => Some(ActorMessageSourceKind::System),
        _ => None,
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

fn payload_bool(payload: &Value, key: &str) -> Option<bool> {
    payload
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_bool)
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

pub fn actor_message_payload_conversation_id(payload: &Value) -> Option<String> {
    for key in [
        "task_conversation_id",
        "channel_conversation_id",
        "conversation_id",
    ] {
        if let Some(value) = payload_trimmed_string(payload, key) {
            return Some(value);
        }
    }
    None
}

pub fn actor_message_payload_source_surface(payload: &Value) -> Option<String> {
    payload_trimmed_string(payload, "source_surface")
}

pub fn actor_message_payload_reply_target(payload: &Value) -> Option<Value> {
    payload
        .as_object()
        .and_then(|map| map.get("reply_target"))
        .filter(|value| !value.is_null())
        .cloned()
}

pub fn actor_message_payload_requires_user_visible_reply(payload: &Value) -> Option<bool> {
    payload_bool(payload, "requires_user_visible_reply")
}

pub fn infer_actor_message_source_kind(
    from_actor_id: &str,
    message_kind: &ActorMessageKind,
    payload: &Value,
) -> ActorMessageSourceKind {
    if let Some(explicit) = payload
        .as_object()
        .and_then(|map| map.get("source_kind"))
        .and_then(Value::as_str)
        .and_then(parse_actor_message_source_kind)
    {
        return explicit;
    }
    match message_kind {
        ActorMessageKind::TriggerEvent => ActorMessageSourceKind::Trigger,
        ActorMessageKind::SystemNotice => ActorMessageSourceKind::System,
        _ => {
            if infer_actor_identity_kind(from_actor_id) == ActorIdentityKind::Human {
                ActorMessageSourceKind::Human
            } else {
                ActorMessageSourceKind::Agent
            }
        }
    }
}

fn infer_actor_message_source_surface(message_kind: &ActorMessageKind, payload: &Value) -> String {
    if let Some(explicit) = actor_message_payload_source_surface(payload) {
        return explicit;
    }
    if actor_message_payload_thread_root_message_id(payload).is_some() {
        return "thread".to_string();
    }
    if actor_message_payload_conversation_id(payload).is_some() {
        return "conversation".to_string();
    }
    match message_kind {
        ActorMessageKind::TriggerEvent => "trigger".to_string(),
        ActorMessageKind::SystemNotice => "system".to_string(),
        _ => "mailbox".to_string(),
    }
}

fn infer_actor_message_reply_target(payload: &Value) -> Option<Value> {
    if let Some(explicit) = actor_message_payload_reply_target(payload) {
        return Some(explicit);
    }
    let conversation_id = actor_message_payload_conversation_id(payload);
    let task_id = actor_message_payload_task_id(payload);
    let task_message_id = payload_positive_i64(payload, "task_message_id");
    let thread_root_message_id = actor_message_payload_thread_root_message_id(payload);
    if conversation_id.is_none() && task_message_id.is_none() && thread_root_message_id.is_none() {
        return None;
    }
    let mut reply_target = Map::new();
    reply_target.insert(
        "surface".to_string(),
        Value::String(if thread_root_message_id.is_some() {
            "thread".to_string()
        } else {
            "conversation".to_string()
        }),
    );
    if let Some(task_id) = task_id {
        reply_target.insert("task_id".to_string(), Value::String(task_id));
    }
    if let Some(conversation_id) = conversation_id {
        reply_target.insert(
            "conversation_id".to_string(),
            Value::String(conversation_id),
        );
    }
    if let Some(task_message_id) = task_message_id {
        reply_target.insert(
            "task_message_id".to_string(),
            Value::Number(task_message_id.into()),
        );
    }
    if let Some(thread_root_message_id) = thread_root_message_id {
        reply_target.insert(
            "thread_root_message_id".to_string(),
            Value::Number(thread_root_message_id.into()),
        );
    }
    Some(Value::Object(reply_target))
}

fn infer_requires_user_visible_reply(
    source_kind: &ActorMessageSourceKind,
    to_actor_id: &str,
    payload: &Value,
) -> bool {
    if let Some(explicit) = actor_message_payload_requires_user_visible_reply(payload) {
        return explicit;
    }
    matches!(source_kind, ActorMessageSourceKind::Human)
        && infer_actor_identity_kind(to_actor_id) == ActorIdentityKind::Agent
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActorMessageEnvelopeProjection {
    pub source_kind: ActorMessageSourceKind,
    pub source_surface: String,
    pub reply_target: Option<Value>,
    pub requires_user_visible_reply: bool,
    pub conversation_id: Option<String>,
    pub thread_root_message_id: Option<i64>,
}

pub fn project_actor_message_envelope(
    from_actor_id: &str,
    to_actor_id: &str,
    message_kind: &ActorMessageKind,
    payload: &Value,
) -> ActorMessageEnvelopeProjection {
    let source_kind = infer_actor_message_source_kind(from_actor_id, message_kind, payload);
    ActorMessageEnvelopeProjection {
        source_surface: infer_actor_message_source_surface(message_kind, payload),
        reply_target: infer_actor_message_reply_target(payload),
        requires_user_visible_reply: infer_requires_user_visible_reply(
            &source_kind,
            to_actor_id,
            payload,
        ),
        conversation_id: actor_message_payload_conversation_id(payload),
        thread_root_message_id: actor_message_payload_thread_root_message_id(payload),
        source_kind,
    }
}

fn parse_stringified_json_value(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn canonical_actor_message_payload_object(
    message_kind: &ActorMessageKind,
    payload: Value,
) -> Map<String, Value> {
    match payload {
        Value::Object(payload_obj) => payload_obj,
        Value::String(text) => {
            if let Some(parsed) = parse_stringified_json_value(&text)
                && let Value::Object(payload_obj) = parsed
            {
                return payload_obj;
            }
            let mut payload_obj = Map::new();
            payload_obj.insert(
                "type".to_string(),
                Value::String("chat_message".to_string()),
            );
            payload_obj.insert("text".to_string(), Value::String(text));
            payload_obj
        }
        other => {
            let mut payload_obj = Map::new();
            payload_obj.insert(
                "type".to_string(),
                Value::String(message_kind.as_str().to_string()),
            );
            payload_obj.insert("payload".to_string(), other);
            payload_obj
        }
    }
}

fn insert_canonical_string(payload_obj: &mut Map<String, Value>, key: &str, value: String) {
    let needs_value = payload_obj
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .is_none_or(str::is_empty);
    if needs_value {
        payload_obj.insert(key.to_string(), Value::String(value));
    }
}

fn insert_canonical_value(payload_obj: &mut Map<String, Value>, key: &str, value: Value) {
    if payload_obj.get(key).is_none_or(Value::is_null) {
        payload_obj.insert(key.to_string(), value);
    }
}

pub fn normalize_actor_message_envelope_payload(
    from_actor_id: &str,
    to_actor_id: &str,
    message_kind: &ActorMessageKind,
    payload: Value,
) -> Value {
    let mut payload_obj = canonical_actor_message_payload_object(message_kind, payload);
    let projection = project_actor_message_envelope(
        from_actor_id,
        to_actor_id,
        message_kind,
        &Value::Object(payload_obj.clone()),
    );
    insert_canonical_string(
        &mut payload_obj,
        "source_kind",
        projection.source_kind.as_str().to_string(),
    );
    insert_canonical_string(
        &mut payload_obj,
        "source_surface",
        projection.source_surface,
    );
    if payload_obj
        .get("requires_user_visible_reply")
        .and_then(Value::as_bool)
        .is_none()
    {
        payload_obj.insert(
            "requires_user_visible_reply".to_string(),
            Value::Bool(projection.requires_user_visible_reply),
        );
    }
    if let Some(conversation_id) = projection.conversation_id {
        insert_canonical_string(&mut payload_obj, "conversation_id", conversation_id);
    }
    if let Some(thread_root_message_id) = projection.thread_root_message_id {
        insert_canonical_value(
            &mut payload_obj,
            "thread_root_message_id",
            Value::Number(thread_root_message_id.into()),
        );
    }
    if let Some(reply_target) = projection.reply_target {
        insert_canonical_value(&mut payload_obj, "reply_target", reply_target);
    }
    Value::Object(payload_obj)
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
    let topic = payload_trimmed_string(payload, "topic");
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
    if let Some(topic) = topic {
        return Some(ActorMessageTopicMetadata {
            topic_key: match task_id.clone() {
                Some(task_id) => format!("task:{task_id}:topic:{topic}"),
                None => format!("topic:{topic}"),
            },
            task_id,
            root_message_id,
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

impl ActorMessageRecord {
    pub fn inbound_envelope(&self) -> ActorMessageEnvelopeProjection {
        project_actor_message_envelope(
            &self.from_actor_id,
            &self.to_actor_id,
            &self.message_kind,
            &self.payload,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActorIdentityKind, ActorMessageEnvelopeProjection, ActorMessageHandlingDisposition,
        ActorMessageKind, ActorMessageSourceKind, ActorMessageTaskRelation, ActorThreadClaimStatus,
        derive_actor_message_topic_metadata, infer_actor_identity_kind, infer_actor_message_kind,
        normalize_actor_message_envelope_payload, parse_actor_message_handling_disposition,
        parse_actor_message_kind, parse_actor_message_source_kind,
        parse_actor_message_task_relation, parse_actor_thread_claim_status,
        project_actor_message_envelope,
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
    fn derive_actor_message_topic_metadata_uses_explicit_topic_before_correlation() {
        let metadata = derive_actor_message_topic_metadata(
            12,
            &json!({
                "topic": "incident/12",
                "correlation_id": "corr-12",
                "task_id": "task-12"
            }),
            None,
        )
        .expect("derive metadata");
        assert_eq!(metadata.topic_key, "task:task-12:topic:incident/12");
        assert_eq!(metadata.task_id.as_deref(), Some("task-12"));
        assert_eq!(metadata.root_message_id, None);
    }

    #[test]
    fn derive_actor_message_topic_metadata_falls_back_to_correlation_scope() {
        let metadata = derive_actor_message_topic_metadata(
            13,
            &json!({
                "correlation_id": "corr-13",
                "task_id": "task-13"
            }),
            None,
        )
        .expect("derive metadata");
        assert_eq!(metadata.topic_key, "correlation:corr-13");
        assert_eq!(metadata.task_id.as_deref(), Some("task-13"));
        assert_eq!(metadata.root_message_id, None);
    }

    #[test]
    fn derive_actor_message_topic_metadata_returns_none_without_scope() {
        assert_eq!(
            derive_actor_message_topic_metadata(14, &json!({"task_id":"   "}), Some("   ")),
            None
        );
        assert_eq!(
            derive_actor_message_topic_metadata(
                15,
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
    fn parse_actor_message_source_kind_accepts_known_values() {
        assert_eq!(
            parse_actor_message_source_kind("human"),
            Some(ActorMessageSourceKind::Human)
        );
        assert_eq!(
            parse_actor_message_source_kind("system"),
            Some(ActorMessageSourceKind::System)
        );
        assert_eq!(parse_actor_message_source_kind("unknown"), None);
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

    #[test]
    fn project_actor_message_envelope_marks_human_conversation_reply_obligation() {
        let projection = project_actor_message_envelope(
            "user:alice",
            "worker-1",
            &ActorMessageKind::HumanRequest,
            &json!({
                "task_id":"task-1",
                "task_conversation_id":"conversation-1",
                "task_message_id": 42
            }),
        );
        assert_eq!(
            projection,
            ActorMessageEnvelopeProjection {
                source_kind: ActorMessageSourceKind::Human,
                source_surface: "conversation".to_string(),
                reply_target: Some(json!({
                    "surface":"conversation",
                    "task_id":"task-1",
                    "conversation_id":"conversation-1",
                    "task_message_id": 42
                })),
                requires_user_visible_reply: true,
                conversation_id: Some("conversation-1".to_string()),
                thread_root_message_id: None,
            }
        );
    }

    #[test]
    fn project_actor_message_envelope_marks_system_and_thread_surfaces() {
        let system_projection = project_actor_message_envelope(
            "planner",
            "reviewer",
            &ActorMessageKind::SystemNotice,
            &json!({
                "type":"permission_review_request"
            }),
        );
        assert_eq!(
            system_projection.source_kind,
            ActorMessageSourceKind::System
        );
        assert_eq!(system_projection.source_surface, "system");
        assert!(!system_projection.requires_user_visible_reply);

        let thread_projection = project_actor_message_envelope(
            "user:bob",
            "worker-1",
            &ActorMessageKind::ThreadReply,
            &json!({
                "task_id":"task-2",
                "task_conversation_id":"conversation-2",
                "task_message_id": 9,
                "thread_root_message_id": 7
            }),
        );
        assert_eq!(thread_projection.source_surface, "thread");
        assert_eq!(
            thread_projection.reply_target,
            Some(json!({
                "surface":"thread",
                "task_id":"task-2",
                "conversation_id":"conversation-2",
                "task_message_id": 9,
                "thread_root_message_id": 7
            }))
        );
        assert_eq!(thread_projection.thread_root_message_id, Some(7));
    }

    #[test]
    fn normalize_actor_message_envelope_payload_backfills_canonical_fields() {
        let normalized = normalize_actor_message_envelope_payload(
            "user:alice",
            "worker-1",
            &ActorMessageKind::HumanRequest,
            json!({
                "task_id":"task-7",
                "task_conversation_id":"conversation-7",
                "task_message_id": 11
            }),
        );
        assert_eq!(normalized["source_kind"], json!("human"));
        assert_eq!(normalized["source_surface"], json!("conversation"));
        assert_eq!(normalized["requires_user_visible_reply"], json!(true));
        assert_eq!(
            normalized["reply_target"],
            json!({
                "surface":"conversation",
                "task_id":"task-7",
                "conversation_id":"conversation-7",
                "task_message_id": 11
            })
        );
    }

    #[test]
    fn normalize_actor_message_envelope_payload_wraps_text_as_canonical_chat() {
        let normalized = normalize_actor_message_envelope_payload(
            "planner",
            "reviewer",
            &ActorMessageKind::CoordinationRequest,
            json!("please review the patch"),
        );
        assert_eq!(normalized["type"], json!("chat_message"));
        assert_eq!(normalized["text"], json!("please review the patch"));
        assert_eq!(normalized["source_kind"], json!("agent"));
        assert_eq!(normalized["source_surface"], json!("mailbox"));
        assert_eq!(normalized["requires_user_visible_reply"], json!(false));
    }

    #[test]
    fn normalize_actor_message_envelope_payload_parses_stringified_json_object() {
        let normalized = normalize_actor_message_envelope_payload(
            "reviewer",
            "user:alice",
            &ActorMessageKind::CoordinationRequest,
            json!(r#"{"type":"chat_message","text":"done","correlation_id":"corr-1"}"#),
        );
        assert_eq!(normalized["type"], json!("chat_message"));
        assert_eq!(normalized["text"], json!("done"));
        assert_eq!(normalized["correlation_id"], json!("corr-1"));
        assert_eq!(normalized["source_kind"], json!("agent"));
        assert_eq!(normalized["source_surface"], json!("mailbox"));
        assert_eq!(normalized["requires_user_visible_reply"], json!(false));
    }

    #[test]
    fn normalize_actor_message_envelope_payload_replaces_blank_or_null_canonical_fields() {
        let normalized = normalize_actor_message_envelope_payload(
            "user:bob",
            "worker-1",
            &ActorMessageKind::HumanRequest,
            json!({
                "type":"chat_message",
                "text":"Need help",
                "source_kind":" ",
                "source_surface":"",
                "requires_user_visible_reply": null,
                "reply_target": null,
                "task_conversation_id":"conversation-9",
                "task_message_id": 5
            }),
        );
        assert_eq!(normalized["source_kind"], json!("human"));
        assert_eq!(normalized["source_surface"], json!("conversation"));
        assert_eq!(normalized["conversation_id"], json!("conversation-9"));
        assert_eq!(normalized["requires_user_visible_reply"], json!(true));
        assert_eq!(
            normalized["reply_target"],
            json!({
                "surface":"conversation",
                "conversation_id":"conversation-9",
                "task_message_id": 5
            })
        );
    }

    #[test]
    fn normalize_actor_message_envelope_payload_replaces_non_boolean_reply_required_flag() {
        let normalized = normalize_actor_message_envelope_payload(
            "coordinator",
            "worker-1",
            &ActorMessageKind::CoordinationRequest,
            json!({
                "type":"chat_message",
                "text":"Review this when free",
                "requires_user_visible_reply":"yes"
            }),
        );
        assert_eq!(normalized["source_kind"], json!("agent"));
        assert_eq!(normalized["source_surface"], json!("mailbox"));
        assert_eq!(normalized["requires_user_visible_reply"], json!(false));
    }

    #[test]
    fn normalize_actor_message_envelope_payload_preserves_boolean_reply_required_flag() {
        let normalized = normalize_actor_message_envelope_payload(
            "worker-1",
            "user:alice",
            &ActorMessageKind::CoordinationRequest,
            json!({
                "type":"chat_message",
                "text":"Operator follow-up needed",
                "requires_user_visible_reply": true
            }),
        );
        assert_eq!(normalized["source_kind"], json!("agent"));
        assert_eq!(normalized["source_surface"], json!("mailbox"));
        assert_eq!(normalized["requires_user_visible_reply"], json!(true));
    }

    #[test]
    fn normalize_actor_message_envelope_payload_marks_trigger_events() {
        let normalized = normalize_actor_message_envelope_payload(
            "timer",
            "worker-1",
            &ActorMessageKind::TriggerEvent,
            json!({
                "payload": {"wake": true}
            }),
        );
        assert_eq!(normalized["source_kind"], json!("trigger"));
        assert_eq!(normalized["source_surface"], json!("trigger"));
        assert_eq!(normalized["requires_user_visible_reply"], json!(false));
    }
}
