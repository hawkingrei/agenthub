use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub to_actor_id: String,
    pub channel: String,
    pub transport: ActorMessageTransport,
    pub route: Option<Value>,
    pub payload: Value,
    pub status: ActorMessageStatus,
    pub created_at: i64,
    pub delivered_at: Option<i64>,
}
