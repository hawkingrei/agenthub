use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorMessageRecord {
    pub message_id: i64,
    pub run_id: String,
    pub from_actor_id: String,
    pub from_actor_kind: ActorIdentityKind,
    pub to_actor_id: String,
    pub to_actor_kind: ActorIdentityKind,
    pub channel: String,
    pub transport: ActorMessageTransport,
    pub route: Option<Value>,
    pub payload: Value,
    pub status: ActorMessageStatus,
    pub created_at: i64,
    pub delivered_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::{ActorIdentityKind, infer_actor_identity_kind};

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
}
