//! Safe operation receipts shared by MCP integrations. Payloads stay in the transport.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpDigest(String);

impl McpDigest {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for McpDigest {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        anyhow::ensure!(
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "MCP journal digest must be a lowercase SHA-256 digest"
        );
        Ok(Self(value))
    }
}

impl From<McpDigest> for String {
    fn from(value: McpDigest) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpOperationStatus {
    Prepared,
    Sent,
    Succeeded,
    Failed,
    OutcomeUnknown,
}

impl McpOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Sent => "sent",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::OutcomeUnknown => "outcome_unknown",
        }
    }

    /// A replay creates a new attempt; it never changes an old attempt back to sent.
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Prepared, Self::Sent | Self::Failed)
                | (
                    Self::Sent,
                    Self::Succeeded | Self::Failed | Self::OutcomeUnknown
                )
        )
    }
}

impl std::str::FromStr for McpOperationStatus {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "sent" => Ok(Self::Sent),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "outcome_unknown" => Ok(Self::OutcomeUnknown),
            _ => anyhow::bail!("invalid MCP operation status"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum McpReplaySafety {
    ReadOnly,
    NonIdempotent,
    /// The trusted binding declares a schema field and the caller supplied its value.
    /// An upstream idempotentHint by itself does not establish this guarantee.
    StableIdentity {
        identity_digest: McpDigest,
    },
}

impl McpReplaySafety {
    pub fn permits_retry(&self) -> bool {
        !matches!(self, Self::NonIdempotent)
    }

    pub fn identity_digest(&self) -> Option<&McpDigest> {
        match self {
            Self::StableIdentity { identity_digest } => Some(identity_digest),
            _ => None,
        }
    }
}

/// The integration establishes stable upstream authority independently of endpoint aliases.
/// Older intents have no such evidence; their digests must not be reinterpreted after a move.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpScopeIdentity {
    #[default]
    Legacy,
    VerifiedAuthority,
}

impl McpScopeIdentity {
    pub fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
}

/// Constructed by trusted proxy policy after scope binding and schema validation.
/// All digests cover canonical values; no raw arguments, identities, or URLs belong here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpOperationIntent {
    pub request_key: McpDigest,
    pub server_id: String,
    pub scope_digest: McpDigest,
    #[serde(default, skip_serializing_if = "McpScopeIdentity::is_legacy")]
    pub scope_identity: McpScopeIdentity,
    pub binding_digest: McpDigest,
    pub tool_name: String,
    pub schema_digest: McpDigest,
    pub arguments_digest: McpDigest,
    /// Present for calls constructed by proxy policy. Older journal checkpoints omitted it.
    #[serde(default)]
    pub request_digest: Option<McpDigest>,
    pub replay_safety: McpReplaySafety,
}

impl McpOperationIntent {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.server_id.is_empty()
                && self.server_id.len() <= 128
                && self
                    .server_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte)),
            "MCP journal requires a bounded server identifier"
        );
        // Upstream naming conventions are recommendations, not a namespace rewrite contract.
        anyhow::ensure!(
            !self.tool_name.is_empty()
                && self.tool_name.len() <= 4096
                && !self.tool_name.chars().any(char::is_control),
            "MCP tool name must be bounded text without control characters"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpFailureKind {
    JsonRpc,
    McpResult,
    SuccessEnvelope,
    TaskCancelled,
}

/// Shared classification for Mem and other MCP integrations. The caller retains the unmodified
/// response; a failure receipt does not prove that an upstream write had no effect.
pub fn classify_response_error(response: &serde_json::Value) -> Option<McpFailureKind> {
    if response.get("error").is_some() {
        return Some(McpFailureKind::JsonRpc);
    }
    if response
        .get("result")
        .and_then(|result| result.get("isError"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        return Some(McpFailureKind::McpResult);
    }
    response
        .get("result")
        .and_then(|result| result.get("error"))
        .is_some()
        .then_some(McpFailureKind::SuccessEnvelope)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAmbiguityReason {
    TransportLost,
    Deadline,
    InvalidResponse,
    DaemonRestart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpDeferralKind {
    InputRequired,
    TaskAccepted,
}

/// Correlation facts only; opaque upstream state and user input remain in the transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpInputReceipt {
    pub state_digest: Option<McpDigest>,
    pub input_ids: Vec<McpDigest>,
    pub request_id_digest: McpDigest,
}

/// Derived from the bound continuation request by trusted proxy policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpContinuationInput {
    pub state_digest: Option<McpDigest>,
    pub input_ids: Vec<McpDigest>,
    pub request_id_digest: McpDigest,
    pub request_digest: McpDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpContinuationRecord {
    pub parent_attempt_number: u32,
    pub parent_response_digest: McpDigest,
    pub request_id_digest: McpDigest,
    pub request_digest: McpDigest,
    /// The first send of this round, when this attempt retries its unchanged parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_of_attempt_number: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTaskVersion {
    November2025,
    July2026,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpTaskReceipt {
    pub task_digest: McpDigest,
    pub version: McpTaskVersion,
    pub session_digest: Option<McpDigest>,
}

/// A trusted integration snapshot for incoming task facts, never caller-supplied metadata.
#[derive(Clone)]
pub struct McpTaskObservationBinding {
    pub server_id: String,
    pub scope_digest: McpDigest,
    pub binding_digest: McpDigest,
}

pub struct McpTaskObservation {
    pub receipt: McpTaskReceipt,
    pub response_digest: McpDigest,
    pub outcome: Option<McpCompletion>,
    pub inputs: Option<Vec<McpTaskInputRequest>>,
}

/// Trusted binding and discovered schemas, never supplied by the provider as authority.
pub struct McpTaskAuthority {
    pub server_id: String,
    pub scope_digest: McpDigest,
    pub binding_digest: McpDigest,
    pub tools: std::collections::BTreeMap<String, McpDigest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTaskLookupMethod {
    Get,
    Result,
}

pub struct McpTaskLookupInput {
    pub receipt: McpTaskReceipt,
    pub method: McpTaskLookupMethod,
    pub request_key: McpDigest,
    pub request_digest: McpDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTaskLookupRecord {
    pub sequence: i64,
    pub id: String,
    pub operation_id: String,
    pub attempt_number: u32,
    pub activation_id: String,
    pub method: McpTaskLookupMethod,
    pub completion: Option<McpCompletion>,
    pub outcome: Option<McpCompletion>,
}

pub struct McpTaskCancellationInput {
    pub receipt: McpTaskReceipt,
    pub request_key: McpDigest,
    pub request_digest: McpDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTaskCancellationRecord {
    pub operation_id: String,
    pub attempt_number: u32,
    pub activation_id: String,
    pub completion: Option<McpCompletion>,
    pub outcome: Option<McpCompletion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTaskInputRequest {
    pub input_id_digest: McpDigest,
    pub request_digest: McpDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTaskInputResponse {
    pub input_id_digest: McpDigest,
    pub response_digest: McpDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTaskNotificationRecord {
    pub sequence: i64,
    pub attempt_number: u32,
    pub activation_id: String,
    pub response_digest: McpDigest,
    pub outcome: Option<McpCompletion>,
    pub inputs_valid: bool,
}

pub struct McpTaskUpdateInput {
    pub receipt: McpTaskReceipt,
    pub request_key: McpDigest,
    pub request_digest: McpDigest,
    pub inputs: Vec<McpTaskInputResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTaskInputRecord {
    pub sequence: i64,
    pub attempt_number: u32,
    pub input_id_digest: McpDigest,
    pub request_digest: McpDigest,
    pub update_id: Option<String>,
    pub conflicted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTaskUpdateRecord {
    pub sequence: i64,
    pub id: String,
    pub operation_id: String,
    pub attempt_number: u32,
    pub activation_id: String,
    pub inputs: Vec<McpTaskInputResponse>,
    pub completion: Option<McpCompletion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum McpCompletion {
    Succeeded {
        response_digest: McpDigest,
    },
    Failed {
        reason: McpFailureKind,
        response_digest: McpDigest,
    },
    OutcomeUnknown {
        reason: McpAmbiguityReason,
    },
    /// The RPC returned a receipt, but no terminal tool outcome. Preserve the fact without
    /// claiming success or authorizing a replay of the initial request.
    Deferred {
        reason: McpDeferralKind,
        response_digest: McpDigest,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_receipt: Option<McpInputReceipt>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task_receipt: Option<McpTaskReceipt>,
    },
}

impl McpCompletion {
    pub fn status(&self) -> McpOperationStatus {
        match self {
            Self::Succeeded { .. } => McpOperationStatus::Succeeded,
            Self::Failed { .. } => McpOperationStatus::Failed,
            Self::OutcomeUnknown { .. } | Self::Deferred { .. } => {
                McpOperationStatus::OutcomeUnknown
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOperationRecord {
    pub id: String,
    pub actor_id: String,
    pub team_id: String,
    pub origin_activation_id: String,
    pub intent: McpOperationIntent,
    pub status: McpOperationStatus,
    pub attempt_count: u32,
    pub completion: Option<McpCompletion>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpAttemptRecord {
    pub operation_id: String,
    pub number: u32,
    pub activation_id: String,
    pub generation: i64,
    pub status: McpOperationStatus,
    pub completion: Option<McpCompletion>,
    pub sent_at: i64,
    pub completed_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation: Option<McpContinuationRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOperationEvent {
    pub id: i64,
    pub operation_id: String,
    pub attempt_number: u32,
    pub activation_id: String,
    pub status: McpOperationStatus,
    pub completion: Option<McpCompletion>,
    pub created_at: i64,
}
