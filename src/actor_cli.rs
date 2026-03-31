use serde_json::Value;

use crate::team::{TeamActorMessageTransport, TeamTaskListQuery, TeamTaskStatus};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID, build_default_actor_channel_idempotency_key,
    build_default_actor_message_idempotency_key,
};

#[cfg(test)]
use crate::actor_runtime_env::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_AGENT_ID_ENV, ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
    ACTOR_RUNTIME_TEAM_ID_ENV,
};
#[cfg(test)]
use agenthub_team_actor::{
    ActorInboxRequest, ActorMailboxService, ActorMessageStatus, ActorServiceError,
    ActorServiceErrorCode,
};

const MAX_TIME_TRIGGER_DELAY_SECONDS: i64 = 30 * 24 * 60 * 60;
const TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS: i64 = 1;
const ACTOR_HELP_TOPIC_INBOX: &str = "inbox";
const ACTOR_HELP_TOPIC_ACK: &str = "ack";
const ACTOR_HELP_TOPIC_SEND: &str = "send";
const ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND: &str = "permission-review-respond";
const ACTOR_HELP_TOPIC_TEAM_TASK_SHOW: &str = "team-task-show";
const ACTOR_HELP_TOPIC_TEAM_TASK_NOTE: &str = "team-task-note";
const ACTOR_HELP_TOPICS: &[&str] = &[
    "team-members",
    "team-tasks",
    "team-task-create",
    "team-task-update",
    ACTOR_HELP_TOPIC_TEAM_TASK_SHOW,
    ACTOR_HELP_TOPIC_TEAM_TASK_NOTE,
    ACTOR_HELP_TOPIC_INBOX,
    ACTOR_HELP_TOPIC_ACK,
    ACTOR_HELP_TOPIC_SEND,
    "time-trigger-set",
    "time-trigger-list",
    "time-trigger-cancel",
    ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorOutputMode {
    Default,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorOutputPreference {
    ToonPreferred,
    JsonPreferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorOutputFormat {
    Toon,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorSendPayloadSource {
    Text,
    Payload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActorSendIdempotency {
    Disabled,
    Resolved(String),
    DeferredDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorSendTargetRef<'a> {
    Direct { to_actor_id: &'a str },
    Channel { channel_id: &'a str },
}

fn build_actor_send_default_idempotency_key(
    run_id: &str,
    from_actor_id: &str,
    target: ActorSendTargetRef<'_>,
    channel: &str,
    transport: &TeamActorMessageTransport,
    route: Option<&Value>,
    payload: &Value,
) -> String {
    match target {
        ActorSendTargetRef::Direct { to_actor_id } => build_default_actor_message_idempotency_key(
            run_id,
            from_actor_id,
            ACTOR_MAIN_PEER_ID,
            to_actor_id,
            if *transport == TeamActorMessageTransport::Remote {
                ACTOR_NODE_PEER_ID
            } else {
                ACTOR_MAIN_PEER_ID
            },
            channel,
            transport.as_str(),
            route,
            payload,
        ),
        ActorSendTargetRef::Channel { channel_id } => build_default_actor_channel_idempotency_key(
            run_id,
            from_actor_id,
            ACTOR_MAIN_PEER_ID,
            channel_id,
            channel,
            transport.as_str(),
            route,
            payload,
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TeamTaskNoteKind {
    Comment,
    Decision,
    Result,
}

impl TeamTaskNoteKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::Decision => "decision",
            Self::Result => "result",
        }
    }
}

#[derive(Debug)]
enum ActorCommand {
    Help {
        topic: Option<&'static str>,
    },
    TeamMembers {
        team_id: Option<String>,
        run_id: Option<String>,
        actor_id: String,
    },
    Inbox {
        run_id: Option<String>,
        actor_id: String,
        limit: i64,
        after_id: Option<i64>,
        include_delivered: bool,
        auto_ack: bool,
    },
    Ack {
        run_id: Option<String>,
        actor_id: String,
        message_ids: Vec<i64>,
    },
    TeamTasks {
        query: TeamTaskListQuery,
        actor_id: String,
    },
    TeamTaskCreate {
        team_id: String,
        actor_id: String,
        title: String,
        status: TeamTaskStatus,
        topic: Option<String>,
        context: Value,
    },
    TeamTaskShow {
        team_id: Option<String>,
        run_id: Option<String>,
        actor_id: String,
        task_id: String,
        message_limit: i64,
    },
    TeamTaskUpdate {
        team_id: String,
        actor_id: String,
        task_ids: Vec<String>,
        status: Option<TeamTaskStatus>,
        assigned_member_id: Option<String>,
        clear_assigned_member_id: bool,
        context: Option<Value>,
        context_merge: Option<Value>,
    },
    TeamTaskNote {
        team_id: Option<String>,
        run_id: Option<String>,
        actor_id: String,
        task_id: Option<String>,
        shared_thread: bool,
        kind: TeamTaskNoteKind,
        text: String,
    },
    TimeTriggerSet {
        actor_id: String,
        delay_seconds: i64,
        message: String,
    },
    TimeTriggerList {
        actor_id: String,
        limit: i64,
    },
    TimeTriggerCancel {
        actor_id: String,
        trigger_id: String,
    },
    PermissionReviewRespond {
        team_id: String,
        actor_id: String,
        permission_ids: Vec<String>,
        option_id: Option<String>,
        outcome: Option<String>,
    },
    Send {
        run_id: Option<String>,
        from_actor_id: String,
        to_actor_id: Option<String>,
        channel_id: Option<String>,
        channel: String,
        transport: TeamActorMessageTransport,
        route: Option<Value>,
        payload: Box<Value>,
        payload_source: ActorSendPayloadSource,
        idempotency: ActorSendIdempotency,
    },
}
mod execute;
mod help;
mod output;
mod parse;
mod runtime;

use self::execute::run_actor_command;
use self::parse::parse_actor_args;

#[cfg(test)]
use self::execute::ack_actor_messages;
#[cfg(test)]
use self::execute::require_shared_thread_task_id;
#[cfg(test)]
use self::execute::resolve_shared_thread_task_id;
#[cfg(test)]
use self::output::{actor_output_preference_for_command, encode_actor_output};
#[cfg(test)]
use self::parse::{compute_time_trigger_fire_at, parse_actor_command};
#[cfg(test)]
use self::runtime::load_actor_inbox;

fn maybe_reject_legacy_actor_mcp_args(args: &[String]) -> Option<anyhow::Result<()>> {
    if args.first().map(String::as_str) == Some("actor-mcp") {
        return Some(Err(anyhow::anyhow!(
            "`agenthub actor-mcp` has been removed. Use `agenthub actor ...` instead."
        )));
    }
    None
}

pub async fn run_from_args(args: &[String]) -> anyhow::Result<()> {
    if let Some(result) = maybe_reject_legacy_actor_mcp_args(args) {
        return result;
    }
    let parsed = parse_actor_args(args)?;
    run_actor_command(parsed.command, parsed.output_mode).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenthub_team_actor::{
        ActorAckRequest, ActorAckResponse, ActorInboxResponse, ActorSendRequest, ActorSendResponse,
    };
    use serde::Serialize;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex as StdMutex, OnceLock};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn restore_env(key: &str, value: Option<String>) {
        if let Some(value) = value {
            unsafe { std::env::set_var(key, value) }
        } else {
            unsafe { std::env::remove_var(key) }
        }
    }

    struct TempActorCliTestFile {
        path: PathBuf,
    }

    impl TempActorCliTestFile {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempActorCliTestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn write_temp_actor_cli_test_file(prefix: &str, contents: &str) -> TempActorCliTestFile {
        let path = std::env::temp_dir().join(format!("agenthub-{prefix}-{}.tmp", Uuid::new_v4()));
        std::fs::write(&path, contents).expect("write temp actor cli test file");
        TempActorCliTestFile { path }
    }

    #[derive(Clone)]
    struct MockMailboxService {
        inbox: Vec<agenthub_team_actor::ActorMessageRecord>,
        acked_ids: Arc<StdMutex<Vec<i64>>>,
    }

    #[derive(Clone)]
    struct FailingAckMailboxService {
        fail_message_id: i64,
    }

    #[async_trait::async_trait]
    impl ActorMailboxService for MockMailboxService {
        async fn actor_send(
            &self,
            _request: ActorSendRequest,
        ) -> Result<ActorSendResponse, ActorServiceError> {
            unreachable!("send is not used in this test")
        }

        async fn actor_inbox(
            &self,
            _request: ActorInboxRequest,
        ) -> Result<ActorInboxResponse, ActorServiceError> {
            Ok(ActorInboxResponse {
                messages: self.inbox.clone(),
                next_cursor: self.inbox.last().map(|item| item.message_id),
                pending_count: self
                    .inbox
                    .iter()
                    .filter(|message| message.status == ActorMessageStatus::Pending)
                    .count() as i64,
            })
        }

        async fn actor_ack(
            &self,
            request: ActorAckRequest,
        ) -> Result<ActorAckResponse, ActorServiceError> {
            self.acked_ids
                .lock()
                .expect("acquire acked_ids mutex")
                .push(request.message_id);
            let message = self
                .inbox
                .iter()
                .find(|item| item.message_id == request.message_id)
                .expect("find acked message")
                .clone();
            Ok(ActorAckResponse {
                message_id: message.message_id,
                state: ActorMessageStatus::Delivered,
                acked_at: 100,
                status_changed: true,
                message: agenthub_team_actor::ActorMessageRecord {
                    status: ActorMessageStatus::Delivered,
                    delivered_at: Some(100),
                    ..message
                },
            })
        }
    }

    #[async_trait::async_trait]
    impl ActorMailboxService for FailingAckMailboxService {
        async fn actor_send(
            &self,
            _request: ActorSendRequest,
        ) -> Result<ActorSendResponse, ActorServiceError> {
            unreachable!("send is not used in this test")
        }

        async fn actor_inbox(
            &self,
            _request: ActorInboxRequest,
        ) -> Result<ActorInboxResponse, ActorServiceError> {
            unreachable!("inbox is not used in this test")
        }

        async fn actor_ack(
            &self,
            request: ActorAckRequest,
        ) -> Result<ActorAckResponse, ActorServiceError> {
            if request.message_id == self.fail_message_id {
                Err(ActorServiceError::new(
                    ActorServiceErrorCode::Conflict,
                    "ack failed",
                ))
            } else {
                Ok(ActorAckResponse {
                    message_id: request.message_id,
                    state: ActorMessageStatus::Delivered,
                    acked_at: 100,
                    status_changed: true,
                    message: mock_inbox_message(request.message_id, ActorMessageStatus::Delivered),
                })
            }
        }
    }

    fn mock_inbox_message(
        message_id: i64,
        status: ActorMessageStatus,
    ) -> agenthub_team_actor::ActorMessageRecord {
        agenthub_team_actor::ActorMessageRecord {
            message_id,
            run_id: "run-1".to_string(),
            from_actor_id: "leader".to_string(),
            from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            from_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
            to_actor_id: "worker".to_string(),
            to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            to_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
            channel: "default".to_string(),
            transport: agenthub_team_actor::ActorMessageTransport::Local,
            route: None,
            payload: serde_json::json!({"type":"chat_message","text":"hello"}),
            status,
            created_at: 1,
            delivered_at: None,
        }
    }

    #[test]
    fn parse_inbox_uses_env_fallback() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-x");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec!["inbox".to_string(), "--limit".to_string(), "5".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox {
                run_id,
                actor_id,
                limit,
                auto_ack,
                ..
            } => {
                assert_eq!(run_id.as_deref(), Some("run-x"));
                assert_eq!(actor_id, "planner");
                assert_eq!(limit, 5);
                assert!(!auto_ack);
            }
            _ => panic!("expected inbox command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_actor_args_accepts_json_flag_before_subcommand() {
        let args = vec![
            "--json".to_string(),
            "inbox".to_string(),
            "--run-id".to_string(),
            "run-x".to_string(),
            "--actor-id".to_string(),
            "planner".to_string(),
        ];
        let parsed = parse_actor_args(&args).expect("parse actor args");
        assert_eq!(parsed.output_mode, ActorOutputMode::Json);
        assert!(matches!(
            parsed.command,
            ActorCommand::Inbox { ref run_id, ref actor_id, .. }
                if run_id.as_deref() == Some("run-x") && actor_id == "planner"
        ));
    }

    #[test]
    fn parse_actor_args_accepts_json_flag_after_subcommand() {
        let args = vec![
            "inbox".to_string(),
            "--json".to_string(),
            "--run-id".to_string(),
            "run-y".to_string(),
            "--actor-id".to_string(),
            "planner".to_string(),
        ];
        let parsed = parse_actor_args(&args).expect("parse actor args");
        assert_eq!(parsed.output_mode, ActorOutputMode::Json);
        assert!(matches!(
            parsed.command,
            ActorCommand::Inbox { ref run_id, ref actor_id, .. }
                if run_id.as_deref() == Some("run-y") && actor_id == "planner"
        ));
    }

    #[test]
    fn parse_inbox_accepts_auto_ack_flag() {
        let _guard = env_lock().blocking_lock();
        let prev_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-auto-ack");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
        }
        let args = vec!["inbox".to_string(), "--auto-ack".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox { auto_ack, .. } => assert!(auto_ack),
            _ => panic!("expected inbox command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_inbox_allows_team_scope_without_current_run_id() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-shared");
            std::env::remove_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
        }
        let args = vec!["inbox".to_string(), "--limit".to_string(), "20".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox {
                run_id,
                actor_id,
                limit,
                ..
            } => {
                assert!(run_id.is_none());
                assert_eq!(actor_id, "planner");
                assert_eq!(limit, 20);
            }
            _ => panic!("expected inbox command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_members_uses_env_fallback() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-members-team");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-team-members");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker-team-members");
        }
        let args = vec!["team-members".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers {
                team_id,
                run_id,
                actor_id,
            } => {
                assert_eq!(team_id.as_deref(), Some("team-members-team"));
                assert_eq!(run_id.as_deref(), Some("run-team-members"));
                assert_eq!(actor_id, "worker-team-members");
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_members_accepts_run_id_flag() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-env-should-be-ignored");
            std::env::set_var(
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
                "run-env-should-be-ignored",
            );
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker-run-explicit");
        }
        let args = vec![
            "team-members".to_string(),
            "--run-id".to_string(),
            "run-explicit".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers {
                team_id,
                run_id,
                actor_id,
            } => {
                assert!(team_id.is_none());
                assert_eq!(run_id.as_deref(), Some("run-explicit"));
                assert_eq!(actor_id, "worker-run-explicit");
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_members_accepts_team_id_flag_without_run() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-env");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-env");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker-team-explicit");
        }
        let args = vec![
            "team-members".to_string(),
            "--team-id".to_string(),
            "team-explicit".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers {
                team_id,
                run_id,
                actor_id,
            } => {
                assert_eq!(team_id.as_deref(), Some("team-explicit"));
                assert!(run_id.is_none());
                assert_eq!(actor_id, "worker-team-explicit");
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_inbox_ignores_legacy_run_env_alias() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::remove_var(ACTOR_RUNTIME_TEAM_ID_ENV);
            std::env::remove_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV);
            std::env::set_var("AGENTHUB_ACTOR_RUN_ID", "run-legacy-only");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
        }
        let args = vec!["inbox".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox {
                run_id, actor_id, ..
            } => {
                assert!(run_id.is_none(), "legacy run env alias should be ignored");
                assert_eq!(actor_id, "planner");
            }
            other => panic!("expected inbox command, got {other:?}"),
        };
        unsafe {
            std::env::remove_var("AGENTHUB_ACTOR_RUN_ID");
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_send_validates_remote_route() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "remote-peer".to_string(),
            "--transport".to_string(),
            "remote".to_string(),
            "--text".to_string(),
            "hi".to_string(),
        ];
        assert!(
            parse_actor_command(&args, &mut ActorOutputMode::Default).is_err(),
            "remote transport must require route-json"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_generates_default_idempotency_key() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-default-key");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "reviewer".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send { idempotency, .. } => {
                let idempotency_key = match idempotency {
                    ActorSendIdempotency::Resolved(idempotency_key) => idempotency_key,
                    other => panic!("expected resolved idempotency key, got {other:?}"),
                };
                assert!(idempotency_key.starts_with("auto:v1:"));
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_allow_duplicate_disables_default_idempotency_key() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-allow-duplicate");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "reviewer".to_string(),
            "--text".to_string(),
            "hello".to_string(),
            "--allow-duplicate".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send { idempotency, .. } => {
                assert!(
                    matches!(idempotency, ActorSendIdempotency::Disabled),
                    "allow duplicate should skip idempotency key"
                );
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_rejects_duplicate_flag_with_explicit_idempotency_key() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-duplicate-invalid");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "reviewer".to_string(),
            "--text".to_string(),
            "hello".to_string(),
            "--idempotency-key".to_string(),
            "k-1".to_string(),
            "--allow-duplicate".to_string(),
        ];
        assert!(
            parse_actor_command(&args, &mut ActorOutputMode::Default).is_err(),
            "allow duplicate and idempotency key should conflict"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_inbox_accepts_agent_id_alias_flag() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "inbox".to_string(),
            "--agent-id".to_string(),
            "planner-agent".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse inbox");
        match parsed {
            ActorCommand::Inbox { actor_id, .. } => {
                assert_eq!(actor_id, "planner-agent");
            }
            _ => panic!("expected inbox command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_inbox_rejects_agent_id_env_fallback() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_AGENT_ID_ENV, "planner-agent");
        }
        let args = vec!["inbox".to_string()];
        let err =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect_err("reject inbox");
        assert!(
            err.to_string().contains("actor_id is required"),
            "unexpected error: {err}"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_ack_rejects_agent_id_env_fallback() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-x");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_AGENT_ID_ENV, "planner-agent");
        }
        let args = vec![
            "ack".to_string(),
            "--message-id".to_string(),
            "42".to_string(),
        ];
        let err =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect_err("reject ack");
        assert!(
            err.to_string().contains("actor_id is required"),
            "unexpected error: {err}"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_ack_accepts_repeated_message_ids() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-ack-batch");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "ack".to_string(),
            "--message-id".to_string(),
            "41".to_string(),
            "--message-id".to_string(),
            "42".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse repeated ack message ids");
        match parsed {
            ActorCommand::Ack {
                run_id,
                actor_id,
                message_ids,
            } => {
                assert_eq!(run_id.as_deref(), Some("run-ack-batch"));
                assert_eq!(actor_id, "worker");
                assert_eq!(message_ids, vec![41, 42]);
            }
            _ => panic!("expected ack command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_ack_accepts_positional_message_ids() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-ack-positional");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
        }
        let args = vec!["ack".to_string(), "41".to_string(), "42".to_string()];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse positional ack message ids");
        match parsed {
            ActorCommand::Ack {
                run_id,
                actor_id,
                message_ids,
            } => {
                assert_eq!(run_id.as_deref(), Some("run-ack-positional"));
                assert_eq!(actor_id, "worker");
                assert_eq!(message_ids, vec![41, 42]);
            }
            _ => panic!("expected ack command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_ack_allows_team_scope_without_current_run_id() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-ack-scope");
            std::env::remove_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
        }
        let args = vec!["ack".to_string(), "41".to_string()];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse ack without current run");
        match parsed {
            ActorCommand::Ack {
                run_id,
                actor_id,
                message_ids,
            } => {
                assert!(run_id.is_none());
                assert_eq!(actor_id, "worker");
                assert_eq!(message_ids, vec![41]);
            }
            _ => panic!("expected ack command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_send_defers_default_idempotency_key_without_current_run_id() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-send-scope");
            std::env::remove_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse send without current run");
        match parsed {
            ActorCommand::Send {
                run_id,
                idempotency,
                ..
            } => {
                assert!(run_id.is_none());
                assert!(matches!(idempotency, ActorSendIdempotency::DeferredDefault));
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_ack_requires_at_least_one_message_id() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-ack-batch");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let err = parse_actor_command(&["ack".to_string()], &mut ActorOutputMode::Default)
            .expect_err("ack without message ids should fail");
        assert!(
            err.to_string()
                .contains("at least one message_id is required")
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_agent_id_alias_flags() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-alias");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--from-agent-id".to_string(),
            "leader-agent".to_string(),
            "--to-agent-id".to_string(),
            "worker-agent".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                from_actor_id,
                to_actor_id,
                channel_id,
                payload_source,
                ..
            } => {
                assert_eq!(from_actor_id, "leader-agent");
                assert_eq!(to_actor_id.as_deref(), Some("worker-agent"));
                assert!(channel_id.is_none());
                assert_eq!(payload_source, ActorSendPayloadSource::Text);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_rejects_agent_id_env_fallback_for_from_actor() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-env");
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_AGENT_ID_ENV, "leader-agent");
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let err =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect_err("reject send");
        assert!(
            err.to_string().contains("from_actor_id is required"),
            "unexpected error: {err}"
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_text_and_preserves_markdown() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-markdown");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let markdown = "## Review\n\n- keep markdown\n- keep spacing\n";
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            markdown.to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                payload,
                payload_source,
                ..
            } => {
                assert_eq!(*payload, Value::String(markdown.to_string()));
                assert_eq!(payload_source, ActorSendPayloadSource::Text);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_text_file_and_preserves_markdown() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-text-file");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let markdown = "## Review\n\n- keep markdown\n- keep spacing\n";
        let path = write_temp_actor_cli_test_file("actor-send-text", markdown);
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text-file".to_string(),
            path.path().display().to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                payload,
                payload_source,
                ..
            } => {
                assert_eq!(*payload, Value::String(markdown.to_string()));
                assert_eq!(payload_source, ActorSendPayloadSource::Text);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_rejects_text_and_payload_json_together() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-conflict");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            "hello".to_string(),
            "--payload-json".to_string(),
            r#"{"text":"hello"}"#.to_string(),
        ];
        let err = match parse_actor_command(&args, &mut ActorOutputMode::Default) {
            Ok(_) => panic!("text and payload should conflict"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("--text/--text-file and --payload-json/--payload-file")
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_rejects_text_and_text_file_together() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
                "run-send-text-file-conflict",
            );
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let path = write_temp_actor_cli_test_file("actor-send-inline-conflict", "hello from file");
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--text".to_string(),
            "hello".to_string(),
            "--text-file".to_string(),
            path.path().display().to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("text and text-file should conflict");
        assert!(err.to_string().contains("--text and --text-file"));
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_rejects_payload_json_and_payload_file_together() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
                "run-send-payload-file-conflict",
            );
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let path =
            write_temp_actor_cli_test_file("actor-send-payload-conflict", r#"{"from":"file"}"#);
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--payload-json".to_string(),
            r#"{"from":"inline"}"#.to_string(),
            "--payload-file".to_string(),
            path.path().display().to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("payload-json and payload-file should conflict");
        assert!(
            err.to_string()
                .contains("--payload-json and --payload-file cannot be used together")
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_payload_file_reports_flag_in_invalid_json_error() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
                "run-send-payload-file-invalid-json",
            );
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let path = write_temp_actor_cli_test_file("actor-send-payload-invalid", "{not-json");
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--payload-file".to_string(),
            path.path().display().to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("invalid payload-file json should fail");
        assert!(err.to_string().contains("invalid --payload-file JSON"));
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_payload_json_marks_payload_source_for_warning() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-payload");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--payload-json".to_string(),
            r#"{"status":"done","result":"ok"}"#.to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send { payload_source, .. } => {
                assert_eq!(payload_source, ActorSendPayloadSource::Payload);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_payload_file_marks_payload_source_for_warning() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-payload-file");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let path = write_temp_actor_cli_test_file(
            "actor-send-payload",
            r#"{"status":"done","result":"ok"}"#,
        );
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--payload-file".to_string(),
            path.path().display().to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send { payload_source, .. } => {
                assert_eq!(payload_source, ActorSendPayloadSource::Payload);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_channel_id_target() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-channel");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--channel-id".to_string(),
            "all".to_string(),
            "--text".to_string(),
            "@worker review this".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse send");
        match parsed {
            ActorCommand::Send {
                to_actor_id,
                channel_id,
                payload_source,
                ..
            } => {
                assert!(to_actor_id.is_none());
                assert_eq!(channel_id.as_deref(), Some("all"));
                assert_eq!(payload_source, ActorSendPayloadSource::Text);
            }
            _ => panic!("expected send command"),
        }
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_send_accepts_direct_alias_and_shared_flag() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-aliases");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
        }

        let direct_args = vec![
            "send".to_string(),
            "--direct".to_string(),
            "worker-a".to_string(),
            "--text".to_string(),
            "please verify".to_string(),
        ];
        let direct = parse_actor_command(&direct_args, &mut ActorOutputMode::Default)
            .expect("parse direct alias send");
        match direct {
            ActorCommand::Send {
                to_actor_id,
                channel_id,
                ..
            } => {
                assert_eq!(to_actor_id.as_deref(), Some("worker-a"));
                assert!(channel_id.is_none());
            }
            _ => panic!("expected send command"),
        }

        let shared_args = vec![
            "send".to_string(),
            "--shared".to_string(),
            "--text".to_string(),
            "status update".to_string(),
        ];
        let shared = parse_actor_command(&shared_args, &mut ActorOutputMode::Default)
            .expect("parse shared alias send");
        match shared {
            ActorCommand::Send {
                to_actor_id,
                channel_id,
                ..
            } => {
                assert!(to_actor_id.is_none());
                assert_eq!(channel_id.as_deref(), Some("all"));
            }
            _ => panic!("expected send command"),
        }

        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[tokio::test]
    async fn load_actor_inbox_keeps_pending_messages_read_only_by_default() {
        let service = MockMailboxService {
            inbox: vec![mock_inbox_message(1, ActorMessageStatus::Pending)],
            acked_ids: Arc::new(StdMutex::new(Vec::new())),
        };
        let response = load_actor_inbox(
            &service,
            ActorInboxRequest {
                run_id: "run-1".to_string(),
                actor_id: "worker".to_string(),
                cursor: None,
                limit: Some(20),
                states: Some(vec![ActorMessageStatus::Pending]),
            },
            false,
        )
        .await
        .expect("load inbox without auto-ack");
        assert_eq!(response.pending_count, 1);
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].status, ActorMessageStatus::Pending);
        assert!(
            service
                .acked_ids
                .lock()
                .expect("acquire acked ids")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn load_actor_inbox_auto_ack_consumes_pending_messages() {
        let service = MockMailboxService {
            inbox: vec![mock_inbox_message(7, ActorMessageStatus::Pending)],
            acked_ids: Arc::new(StdMutex::new(Vec::new())),
        };
        let response = load_actor_inbox(
            &service,
            ActorInboxRequest {
                run_id: "run-1".to_string(),
                actor_id: "worker".to_string(),
                cursor: None,
                limit: Some(20),
                states: Some(vec![ActorMessageStatus::Pending]),
            },
            true,
        )
        .await
        .expect("load inbox with auto-ack");
        assert_eq!(response.pending_count, 0);
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].status, ActorMessageStatus::Delivered);
        assert_eq!(
            *service.acked_ids.lock().expect("acquire acked ids"),
            vec![7]
        );
    }

    #[tokio::test]
    async fn ack_actor_messages_batches_requests_in_order() {
        let service = MockMailboxService {
            inbox: vec![
                mock_inbox_message(11, ActorMessageStatus::Pending),
                mock_inbox_message(12, ActorMessageStatus::Pending),
            ],
            acked_ids: Arc::new(StdMutex::new(Vec::new())),
        };
        let responses = ack_actor_messages(&service, "run-1", "worker", &[11, 12])
            .await
            .expect("batch ack should succeed");
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].message_id, 11);
        assert_eq!(responses[1].message_id, 12);
        assert_eq!(
            *service.acked_ids.lock().expect("acquire acked ids"),
            vec![11, 12]
        );
    }

    #[tokio::test]
    async fn ack_actor_messages_reports_failed_message_id_in_context() {
        let service = FailingAckMailboxService {
            fail_message_id: 12,
        };
        let err = ack_actor_messages(&service, "run-1", "worker", &[11, 12])
            .await
            .expect_err("batch ack should fail on configured message id");
        assert!(err.to_string().contains("failed to ack message_id=12"));
        assert!(format!("{err:#}").contains("actor ack failed"));
    }

    #[test]
    fn legacy_actor_mcp_entrypoint_is_rejected() {
        let args = vec!["actor-mcp".to_string()];
        let err = maybe_reject_legacy_actor_mcp_args(&args)
            .expect("legacy actor-mcp should be rejected")
            .expect_err("legacy actor-mcp should return an error");
        assert_eq!(
            err.to_string(),
            "`agenthub actor-mcp` has been removed. Use `agenthub actor ...` instead."
        );
    }

    #[test]
    fn parse_send_rejects_conflicting_actor_and_channel_targets() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-send-conflict-target");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "send".to_string(),
            "--to-actor-id".to_string(),
            "worker".to_string(),
            "--channel-id".to_string(),
            "all".to_string(),
            "--text".to_string(),
            "hello".to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("conflicting send targets should fail");
        assert!(err.to_string().contains("cannot be used together"));
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_help_command_is_supported() {
        for arg in ["help", "--help", "-h"] {
            let args = vec![arg.to_string()];
            let parsed =
                parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse help");
            assert!(matches!(parsed, ActorCommand::Help { topic: None }));
        }
    }

    #[test]
    fn parse_help_topic_supports_fuzzy_match() {
        let args = vec!["help".to_string(), "perm".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse help topic");
        assert!(matches!(
            parsed,
            ActorCommand::Help {
                topic: Some(ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND)
            }
        ));
    }

    #[test]
    fn parse_subcommand_help_is_supported() {
        let args = vec!["ack".to_string(), "--help".to_string()];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse subcommand help");
        assert!(matches!(
            parsed,
            ActorCommand::Help {
                topic: Some(ACTOR_HELP_TOPIC_ACK)
            }
        ));
    }

    #[test]
    fn parse_subcommand_positional_help_is_supported() {
        let args = vec!["ack".to_string(), "help".to_string()];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse positional subcommand help");
        assert!(matches!(
            parsed,
            ActorCommand::Help {
                topic: Some(ACTOR_HELP_TOPIC_ACK)
            }
        ));
    }

    #[test]
    fn parse_team_members_allows_help_as_flag_value() {
        let _guard = env_lock().blocking_lock();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker-help");
        }
        let args = vec![
            "team-members".to_string(),
            "--team-id".to_string(),
            "help".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-members with help value");
        match parsed {
            ActorCommand::TeamMembers {
                team_id,
                run_id,
                actor_id,
            } => {
                assert_eq!(team_id.as_deref(), Some("help"));
                assert!(run_id.is_none());
                assert_eq!(actor_id, "worker-help");
            }
            other => panic!("expected team-members command, got {other:?}"),
        }
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_tasks_uses_env_fallback_and_status_filter() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-kanban");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "team-tasks".to_string(),
            "--status".to_string(),
            "in_review".to_string(),
            "--include-shared-thread".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-tasks");
        match parsed {
            ActorCommand::TeamTasks { query, actor_id } => {
                assert_eq!(query.team_id.as_deref(), Some("team-kanban"));
                assert!(query.run_id.is_none());
                assert_eq!(actor_id, "leader");
                assert_eq!(query.status, Some(TeamTaskStatus::InReview));
                assert!(query.include_shared_thread);
            }
            _ => panic!("expected team-tasks command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_team_tasks_accepts_run_scoped_filters() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-env-ignored");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-env-ignored");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader-run-query");
        }
        let args = vec![
            "team-tasks".to_string(),
            "--run-id".to_string(),
            "run-explicit".to_string(),
            "--task-id".to_string(),
            "task-7".to_string(),
            "--assigned-member-id".to_string(),
            "worker-2".to_string(),
            "--topic".to_string(),
            "kanban".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-tasks");
        match parsed {
            ActorCommand::TeamTasks { query, actor_id } => {
                assert!(query.team_id.is_none());
                assert_eq!(query.run_id.as_deref(), Some("run-explicit"));
                assert_eq!(query.task_id.as_deref(), Some("task-7"));
                assert_eq!(query.assigned_member_id.as_deref(), Some("worker-2"));
                assert_eq!(query.topic.as_deref(), Some("kanban"));
                assert_eq!(actor_id, "leader-run-query");
            }
            _ => panic!("expected team-tasks command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_task_create_accepts_context_and_status() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-create");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "team-task-create".to_string(),
            "--title".to_string(),
            "Investigate relay drift".to_string(),
            "--status".to_string(),
            "in_progress".to_string(),
            "--context-json".to_string(),
            r#"{"area":"relay"}"#.to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-task-create");
        match parsed {
            ActorCommand::TeamTaskCreate {
                team_id,
                actor_id,
                title,
                status,
                context,
                ..
            } => {
                assert_eq!(team_id, "team-create");
                assert_eq!(actor_id, "leader");
                assert_eq!(title, "Investigate relay drift");
                assert_eq!(status, TeamTaskStatus::InProgress);
                assert_eq!(context["area"], "relay");
            }
            _ => panic!("expected team-task-create command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_team_task_create_accepts_context_json_file() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let temp_path = std::env::temp_dir().join(format!(
            "agenthub-team-task-create-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temp_path, r#"{"area":"file"}"#).expect("write temp context");
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-create-file");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
        }
        let args = vec![
            "team-task-create".to_string(),
            "--title".to_string(),
            "Use file context".to_string(),
            "--context-json-file".to_string(),
            temp_path.display().to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-task-create");
        match parsed {
            ActorCommand::TeamTaskCreate { context, .. } => {
                assert_eq!(context["area"], "file");
            }
            _ => panic!("expected team-task-create command"),
        }
        let _ = std::fs::remove_file(temp_path);
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_task_show_accepts_run_scope() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-show-ignored");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-show-ignored");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader-show");
        }
        let args = vec![
            "team-task-show".to_string(),
            "--run-id".to_string(),
            "run-7".to_string(),
            "--task-id".to_string(),
            "task-7".to_string(),
            "--message-limit".to_string(),
            "5".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-task-show");
        match parsed {
            ActorCommand::TeamTaskShow {
                team_id,
                run_id,
                actor_id,
                task_id,
                message_limit,
            } => {
                assert!(team_id.is_none());
                assert_eq!(run_id.as_deref(), Some("run-7"));
                assert_eq!(actor_id, "leader-show");
                assert_eq!(task_id, "task-7");
                assert_eq!(message_limit, 5);
            }
            _ => panic!("expected team-task-show command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_task_update_accepts_assignment_patch() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-update");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "team-task-update".to_string(),
            "--task-id".to_string(),
            "task-1".to_string(),
            "--assigned-member-id".to_string(),
            "worker-1".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-task-update");
        match parsed {
            ActorCommand::TeamTaskUpdate {
                team_id,
                actor_id,
                task_ids,
                status,
                assigned_member_id,
                clear_assigned_member_id,
                context,
                context_merge,
            } => {
                assert_eq!(team_id, "team-update");
                assert_eq!(actor_id, "leader");
                assert_eq!(task_ids, vec!["task-1".to_string()]);
                assert!(status.is_none());
                assert_eq!(assigned_member_id.as_deref(), Some("worker-1"));
                assert!(!clear_assigned_member_id);
                assert!(context.is_none());
                assert!(context_merge.is_none());
            }
            _ => panic!("expected team-task-update command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_team_task_update_accepts_batch_context_merge() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-update");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
        }
        let args = vec![
            "team-task-update".to_string(),
            "--task-id".to_string(),
            "task-1".to_string(),
            "--task-id".to_string(),
            "task-2".to_string(),
            "--context-merge-json".to_string(),
            r#"{"repo":"agenthub"}"#.to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-task-update");
        match parsed {
            ActorCommand::TeamTaskUpdate {
                task_ids,
                context,
                context_merge,
                ..
            } => {
                assert_eq!(task_ids, vec!["task-1".to_string(), "task-2".to_string()]);
                assert!(context.is_none());
                assert_eq!(
                    context_merge
                        .as_ref()
                        .and_then(|value| value.get("repo"))
                        .and_then(Value::as_str),
                    Some("agenthub")
                );
            }
            _ => panic!("expected team-task-update command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_task_update_accepts_context_merge_file_alias() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-update-file");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
        }
        let temp = write_temp_actor_cli_test_file("team-task-merge", r#"{"repo":"tidb"}"#);
        let args = vec![
            "team-task-update".to_string(),
            "--task-id".to_string(),
            "task-1".to_string(),
            "--context-merge-file".to_string(),
            temp.path().display().to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-task-update merge file alias");
        match parsed {
            ActorCommand::TeamTaskUpdate { context_merge, .. } => {
                assert_eq!(
                    context_merge
                        .as_ref()
                        .and_then(|value| value.get("repo"))
                        .and_then(Value::as_str),
                    Some("tidb")
                );
            }
            _ => panic!("expected team-task-update command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_task_update_rejects_assign_and_unassign_together() {
        let args = vec![
            "team-task-update".to_string(),
            "--task-id".to_string(),
            "task-1".to_string(),
            "--assigned-member-id".to_string(),
            "worker-1".to_string(),
            "--unassign".to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("reject conflicting assignee flags");
        assert!(
            err.to_string()
                .contains("--assigned-member-id and --unassign cannot be used together")
        );
    }

    #[test]
    fn parse_team_task_update_rejects_blank_task_ids() {
        let args = vec![
            "team-task-update".to_string(),
            "--team-id".to_string(),
            "team-1".to_string(),
            "--actor-id".to_string(),
            "leader".to_string(),
            "--task-id".to_string(),
            "   ".to_string(),
            "--status".to_string(),
            "in_progress".to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("reject blank task ids");
        assert!(err.to_string().contains("task_id is required"));
    }

    #[test]
    fn parse_team_task_note_accepts_kind_and_run_scope() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::remove_var(ACTOR_RUNTIME_TEAM_ID_ENV);
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-note");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader-note");
        }
        let args = vec![
            "team-task-note".to_string(),
            "--task-id".to_string(),
            "task-note-1".to_string(),
            "--kind".to_string(),
            "result".to_string(),
            "--text".to_string(),
            "done".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-task-note");
        match parsed {
            ActorCommand::TeamTaskNote {
                team_id,
                run_id,
                actor_id,
                task_id,
                shared_thread,
                kind,
                text,
            } => {
                assert!(team_id.is_none());
                assert_eq!(run_id.as_deref(), Some("run-note"));
                assert_eq!(actor_id, "leader-note");
                assert_eq!(task_id.as_deref(), Some("task-note-1"));
                assert!(!shared_thread);
                assert_eq!(kind.as_str(), "result");
                assert_eq!(text, "done");
            }
            _ => panic!("expected team-task-note command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_task_note_accepts_shared_thread_without_task_id() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-shared");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader-shared");
        }
        let args = vec![
            "team-task-note".to_string(),
            "--shared-thread".to_string(),
            "--kind".to_string(),
            "result".to_string(),
            "--text".to_string(),
            "shared update".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse shared-thread team-task-note");
        match parsed {
            ActorCommand::TeamTaskNote {
                team_id,
                run_id,
                actor_id,
                task_id,
                shared_thread,
                ..
            } => {
                assert_eq!(team_id.as_deref(), Some("team-shared"));
                assert!(run_id.is_none());
                assert_eq!(actor_id, "leader-shared");
                assert!(task_id.is_none());
                assert!(shared_thread);
            }
            _ => panic!("expected team-task-note command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_team_task_note_rejects_shared_thread_with_explicit_task_id() {
        let args = vec![
            "team-task-note".to_string(),
            "--team-id".to_string(),
            "team-1".to_string(),
            "--actor-id".to_string(),
            "leader".to_string(),
            "--shared-thread".to_string(),
            "--task-id".to_string(),
            "task-1".to_string(),
            "--text".to_string(),
            "oops".to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("shared-thread and task-id should conflict");
        assert!(
            err.to_string()
                .contains("--shared-thread and --task-id cannot be used together")
        );
    }

    #[test]
    fn parse_team_task_note_requires_task_id_or_shared_thread() {
        let args = vec![
            "team-task-note".to_string(),
            "--team-id".to_string(),
            "team-1".to_string(),
            "--actor-id".to_string(),
            "leader".to_string(),
            "--text".to_string(),
            "missing target".to_string(),
        ];
        let err =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect_err("missing target");
        assert!(
            err.to_string()
                .contains("team-task-note requires --task-id or --shared-thread")
        );
    }

    #[test]
    fn resolve_shared_thread_task_id_prefers_canonical_shared_thread_task() {
        let tasks = vec![
            crate::team::TeamTaskRecord {
                id: "task-1".to_string(),
                team_id: "team-1".to_string(),
                title: "Investigate bug".to_string(),
                status: TeamTaskStatus::Open,
                created_by_actor_id: "leader".to_string(),
                assigned_member_id: None,
                context: serde_json::json!({}),
                created_at: 1,
                updated_at: 1,
            },
            crate::team::TeamTaskRecord {
                id: "shared-task".to_string(),
                team_id: "team-1".to_string(),
                title: "all".to_string(),
                status: TeamTaskStatus::Open,
                created_by_actor_id: "leader".to_string(),
                assigned_member_id: None,
                context: serde_json::json!({"bootstrap_kind":"shared_thread"}),
                created_at: 2,
                updated_at: 2,
            },
        ];
        assert_eq!(resolve_shared_thread_task_id(&tasks), Some("shared-task"));
    }

    #[test]
    fn resolve_shared_thread_task_id_prefers_bootstrap_kind_then_latest_update() {
        let tasks = vec![
            crate::team::TeamTaskRecord {
                id: "title-only-newer".to_string(),
                team_id: "team-1".to_string(),
                title: "all".to_string(),
                status: TeamTaskStatus::Open,
                created_by_actor_id: "leader".to_string(),
                assigned_member_id: None,
                context: serde_json::json!({}),
                created_at: 3,
                updated_at: 30,
            },
            crate::team::TeamTaskRecord {
                id: "bootstrap-older".to_string(),
                team_id: "team-1".to_string(),
                title: "all".to_string(),
                status: TeamTaskStatus::Open,
                created_by_actor_id: "leader".to_string(),
                assigned_member_id: None,
                context: serde_json::json!({"bootstrap_kind":"shared_thread"}),
                created_at: 2,
                updated_at: 20,
            },
            crate::team::TeamTaskRecord {
                id: "bootstrap-newer".to_string(),
                team_id: "team-1".to_string(),
                title: "random".to_string(),
                status: TeamTaskStatus::Open,
                created_by_actor_id: "leader".to_string(),
                assigned_member_id: None,
                context: serde_json::json!({"bootstrap_kind":"shared_thread"}),
                created_at: 4,
                updated_at: 40,
            },
        ];
        assert_eq!(
            resolve_shared_thread_task_id(&tasks),
            Some("bootstrap-newer")
        );
    }

    #[test]
    fn require_shared_thread_task_id_returns_error_when_missing() {
        let tasks = vec![crate::team::TeamTaskRecord {
            id: "task-1".to_string(),
            team_id: "team-1".to_string(),
            title: "Investigate bug".to_string(),
            status: TeamTaskStatus::Open,
            created_by_actor_id: "leader".to_string(),
            assigned_member_id: None,
            context: serde_json::json!({}),
            created_at: 1,
            updated_at: 1,
        }];

        let err =
            require_shared_thread_task_id("team-1", &tasks).expect_err("missing shared thread");
        assert!(
            err.to_string()
                .contains("shared thread is missing for team team-1"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn require_shared_thread_task_id_reuses_canonical_selection() {
        let tasks = vec![
            crate::team::TeamTaskRecord {
                id: "task-1".to_string(),
                team_id: "team-1".to_string(),
                title: "all".to_string(),
                status: TeamTaskStatus::Open,
                created_by_actor_id: "leader".to_string(),
                assigned_member_id: None,
                context: serde_json::json!({}),
                created_at: 1,
                updated_at: 10,
            },
            crate::team::TeamTaskRecord {
                id: "shared-task".to_string(),
                team_id: "team-1".to_string(),
                title: "random".to_string(),
                status: TeamTaskStatus::Open,
                created_by_actor_id: "leader".to_string(),
                assigned_member_id: None,
                context: serde_json::json!({"bootstrap_kind":"shared_thread"}),
                created_at: 2,
                updated_at: 2,
            },
        ];

        assert_eq!(
            require_shared_thread_task_id("team-1", &tasks).expect("resolve shared thread"),
            "shared-task"
        );
    }

    #[test]
    fn parse_team_task_update_rejects_duplicate_context_file_aliases() {
        let path = std::env::temp_dir().join(format!(
            "agenthub-context-alias-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, "{}").expect("write temp json fixture");
        let args = vec![
            "team-task-update".to_string(),
            "--team-id".to_string(),
            "team-1".to_string(),
            "--actor-id".to_string(),
            "leader".to_string(),
            "--task-id".to_string(),
            "task-1".to_string(),
            "--context-json-file".to_string(),
            path.display().to_string(),
            "--context-file".to_string(),
            path.display().to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("duplicate context file aliases should conflict");
        let _ = std::fs::remove_file(&path);
        assert!(err.to_string().contains(
            "--context-json, --context-json-file, and --context-file cannot be used together"
        ));
    }

    #[test]
    fn parse_ack_requires_message_id_from_flag_or_position() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-ack-empty");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
        }
        let args = vec!["ack".to_string()];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("missing ack ids should fail");
        assert!(
            err.to_string()
                .contains("at least one message_id is required")
        );
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
    }

    #[test]
    fn parse_time_trigger_set_uses_actor_env_fallback() {
        let _guard = env_lock().blocking_lock();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "time-trigger-set".to_string(),
            "--delay-seconds".to_string(),
            "120".to_string(),
            "--message".to_string(),
            "follow up".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse time-trigger-set");
        match parsed {
            ActorCommand::TimeTriggerSet {
                actor_id,
                delay_seconds,
                message,
            } => {
                assert_eq!(actor_id, "worker");
                assert_eq!(delay_seconds, 120);
                assert_eq!(message, "follow up");
            }
            _ => panic!("expected time-trigger-set command"),
        }
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn compute_time_trigger_fire_at_adds_future_safety_margin() {
        assert_eq!(
            compute_time_trigger_fire_at(1_700_000_000, 1),
            1_700_000_002
        );
        assert_eq!(
            compute_time_trigger_fire_at(1_700_000_000, MAX_TIME_TRIGGER_DELAY_SECONDS),
            1_700_000_000
                + MAX_TIME_TRIGGER_DELAY_SECONDS
                + TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS
        );
    }

    #[test]
    fn parse_permission_review_respond_rejects_conflicting_outcome_flags() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-review");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "permission-review-respond".to_string(),
            "--permission-id".to_string(),
            "perm-1".to_string(),
            "--option-id".to_string(),
            "allow".to_string(),
            "--outcome".to_string(),
            "cancelled".to_string(),
        ];
        let err = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect_err("conflicting permission review flags should fail");
        assert!(err.to_string().contains("cannot be used together"));
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_permission_review_respond_accepts_repeated_permission_ids() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-review");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let args = vec![
            "permission-review-respond".to_string(),
            "--permission-id".to_string(),
            "perm-1".to_string(),
            "--permission-id".to_string(),
            "perm-2".to_string(),
            "--option-id".to_string(),
            "allow".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse repeated permission review ids");
        match parsed {
            ActorCommand::PermissionReviewRespond {
                team_id,
                actor_id,
                permission_ids,
                option_id,
                outcome,
            } => {
                assert_eq!(team_id, "team-review");
                assert_eq!(actor_id, "leader");
                assert_eq!(permission_ids, vec!["perm-1", "perm-2"]);
                assert_eq!(option_id.as_deref(), Some("allow"));
                assert_eq!(outcome, None);
            }
            _ => panic!("expected permission-review-respond command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[test]
    fn parse_permission_review_respond_requires_at_least_one_permission_id() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_agent = std::env::var(ACTOR_RUNTIME_AGENT_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-review");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "leader");
            std::env::remove_var(ACTOR_RUNTIME_AGENT_ID_ENV);
        }
        let err = parse_actor_command(
            &["permission-review-respond".to_string()],
            &mut ActorOutputMode::Default,
        )
        .expect_err("permission-review-respond without ids should fail");
        assert!(
            err.to_string()
                .contains("at least one --permission-id is required")
        );
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
    }

    #[derive(Serialize)]
    struct OutputFixture {
        name: &'static str,
        count: i32,
    }

    #[test]
    fn encode_actor_output_defaults_read_results_to_toon() {
        let output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Default,
            ActorOutputPreference::ToonPreferred,
        )
        .expect("encode toon output");
        assert!(output.contains("name: alpha"));
        assert!(output.contains("count: 2"));
        assert!(!output.starts_with('{'));
    }

    #[test]
    fn encode_actor_output_defaults_confirmation_results_to_json() {
        let output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Default,
            ActorOutputPreference::JsonPreferred,
        )
        .expect("encode json output");
        assert_eq!(output, r#"{"name":"alpha","count":2}"#);
    }

    #[test]
    fn encode_actor_output_json_flag_forces_json() {
        let output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Json,
            ActorOutputPreference::ToonPreferred,
        )
        .expect("encode forced json output");
        assert_eq!(output, r#"{"name":"alpha","count":2}"#);
    }

    #[test]
    fn encode_actor_output_keeps_inbox_cursor_visible() {
        let output = encode_actor_output(
            &ActorInboxResponse {
                messages: Vec::new(),
                next_cursor: Some(42),
                pending_count: 3,
            },
            ActorOutputMode::Default,
            ActorOutputPreference::ToonPreferred,
        )
        .expect("encode inbox response");
        assert!(output.contains("next_cursor: 42"));
        assert!(output.contains("pending_count: 3"));
    }

    #[test]
    fn encode_actor_output_keeps_ack_status_changed_visible() {
        let output = encode_actor_output(
            &ActorAckResponse {
                message_id: 42,
                state: ActorMessageStatus::Delivered,
                acked_at: 100,
                status_changed: false,
                message: mock_inbox_message(42, ActorMessageStatus::Delivered),
            },
            ActorOutputMode::Default,
            ActorOutputPreference::JsonPreferred,
        )
        .expect("encode ack response");
        assert!(output.contains("\"status_changed\":false"));
    }

    #[test]
    fn actor_output_preference_contract_covers_all_command_variants() {
        let cases = vec![
            (
                ActorCommand::Help {
                    topic: Some(ACTOR_HELP_TOPIC_INBOX),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamMembers {
                    team_id: Some("team-1".to_string()),
                    run_id: Some("run-1".to_string()),
                    actor_id: "worker".to_string(),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTasks {
                    query: TeamTaskListQuery {
                        team_id: Some("team-1".to_string()),
                        run_id: None,
                        limit: 10,
                        status: Some(TeamTaskStatus::Open),
                        task_id: None,
                        assigned_member_id: None,
                        topic: None,
                        include_shared_thread: true,
                    },
                    actor_id: "leader".to_string(),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTaskCreate {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    title: "Create task".to_string(),
                    status: TeamTaskStatus::Open,
                    topic: None,
                    context: Value::Object(Default::default()),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTaskShow {
                    team_id: Some("team-1".to_string()),
                    run_id: None,
                    actor_id: "leader".to_string(),
                    task_id: "task-1".to_string(),
                    message_limit: 10,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTaskUpdate {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    task_ids: vec!["task-1".to_string()],
                    status: Some(TeamTaskStatus::InProgress),
                    assigned_member_id: None,
                    clear_assigned_member_id: false,
                    context: None,
                    context_merge: None,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTaskNote {
                    team_id: Some("team-1".to_string()),
                    run_id: None,
                    actor_id: "leader".to_string(),
                    task_id: Some("task-1".to_string()),
                    shared_thread: false,
                    kind: TeamTaskNoteKind::Comment,
                    text: "progress".to_string(),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::Inbox {
                    run_id: Some("run-1".to_string()),
                    actor_id: "worker".to_string(),
                    limit: 20,
                    after_id: None,
                    include_delivered: false,
                    auto_ack: false,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::Ack {
                    run_id: Some("run-1".to_string()),
                    actor_id: "worker".to_string(),
                    message_ids: vec![42],
                },
                ActorOutputPreference::JsonPreferred,
            ),
            (
                ActorCommand::Send {
                    run_id: Some("run-1".to_string()),
                    from_actor_id: "leader".to_string(),
                    to_actor_id: Some("worker".to_string()),
                    channel_id: None,
                    channel: "default".to_string(),
                    transport: TeamActorMessageTransport::Local,
                    route: None,
                    payload: Box::new(Value::String("hello".to_string())),
                    payload_source: ActorSendPayloadSource::Text,
                    idempotency: ActorSendIdempotency::Disabled,
                },
                ActorOutputPreference::JsonPreferred,
            ),
            (
                ActorCommand::TimeTriggerSet {
                    actor_id: "leader".to_string(),
                    delay_seconds: 60,
                    message: "follow up".to_string(),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TimeTriggerList {
                    actor_id: "leader".to_string(),
                    limit: 5,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TimeTriggerCancel {
                    actor_id: "leader".to_string(),
                    trigger_id: "trigger-1".to_string(),
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::PermissionReviewRespond {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    permission_ids: vec!["perm-1".to_string()],
                    option_id: Some("allow".to_string()),
                    outcome: None,
                },
                ActorOutputPreference::JsonPreferred,
            ),
        ];

        for (command, expected) in cases {
            assert_eq!(
                actor_output_preference_for_command(&command),
                expected,
                "unexpected output preference for command variant: {command:?}"
            );
        }

        let toon_output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Default,
            actor_output_preference_for_command(&ActorCommand::TeamMembers {
                team_id: Some("team-1".to_string()),
                run_id: Some("run-1".to_string()),
                actor_id: "worker".to_string(),
            }),
        )
        .expect("encode default team-members output");
        assert!(toon_output.contains("name: alpha"));
        assert!(toon_output.contains("count: 2"));
        assert!(!toon_output.starts_with('{'));
    }

    #[test]
    fn json_flag_still_forces_team_members_json_output() {
        let output = encode_actor_output(
            &OutputFixture {
                name: "alpha",
                count: 2,
            },
            ActorOutputMode::Json,
            actor_output_preference_for_command(&ActorCommand::TeamMembers {
                team_id: Some("team-1".to_string()),
                run_id: Some("run-1".to_string()),
                actor_id: "worker".to_string(),
            }),
        )
        .expect("encode forced json team-members output");
        assert_eq!(output, r#"{"name":"alpha","count":2}"#);
    }
}
