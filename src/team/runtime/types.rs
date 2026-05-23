use serde::{Deserialize, Serialize};

use crate::team::TeamRuntimeStatus;

#[derive(Debug, thiserror::Error)]
pub enum TeamRuntimeStartError {
    #[error("{0}")]
    InvalidConfig(String),
    #[error("{0}")]
    MissingMemberAgent(String),
    #[error("{0}")]
    MemberRuntimeStart(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamRuntimeMemberStatusRecord {
    pub member_id: String,
    pub session_id: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamRuntimeControlRecord {
    pub team_id: String,
    pub status: TeamRuntimeStatus,
    pub members: Vec<TeamRuntimeMemberStatusRecord>,
}
