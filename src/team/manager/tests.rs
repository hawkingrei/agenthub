#![allow(unused_imports)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod archive_cases;
mod cancel_cases;
mod channel_cases;
mod conversation_cases;
mod linked_run_cases;
mod mailbox_basic_cases;
mod mailbox_channel_cases;
mod mailbox_remote_cases;
mod reconcile_cases;
mod run_admin_cases;
mod run_creation_cases;
mod run_lifecycle_cases;
mod runtime_view_cases;
mod step_lifecycle_cases;
mod task_cases;
mod team_cases;
#[path = "tests_support.rs"]
mod tests_support;

use super::codec_status::team_run_status_from_str;
use super::{TeamManager, message_archive_body_text, task_conversation_payload_correlation_id};
use crate::acp::{AcpActorSkillContext, DEFAULT_ACTOR_CHANNEL};
use crate::agent::{WorktreeMode, derive_team_runtime_workdir};
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::InternalGrpcSecurityMode;
use crate::team::{
    SendActorMessageInput, TeamActorMessageStatus, TeamActorMessageTransport, TeamDefinitionConfig,
    TeamRunEventRecord, TeamRunResumeError, TeamRunStatus, TeamStepStatus,
    TeamTaskAssignmentUpdate, TeamTaskContextPatch, TeamTaskListQuery, TeamTaskStatus,
};
use agenthub_db::AgentEventDbRouter;
use agenthub_message_archive::{
    MessageArchiveStore, MessageDocument, MessageDocumentKind, MessageSearchHit, MessageSearchQuery,
};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, ActorAckRequest, ActorIdentityKind, ActorInboxRequest,
    ActorMailboxService, ActorMessageHandlingDisposition, ActorMessageTaskRelation,
    ActorSendRequest, ActorServiceErrorCode, ActorTaskLinkRequest, ActorTriageRequest,
};
use async_trait::async_trait;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::routing::any;
use axum::{Router, serve};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

pub(super) use tests_support::*;
