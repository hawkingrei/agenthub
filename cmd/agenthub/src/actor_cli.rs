use serde_json::Value;

use crate::team::{TeamActorMessageTransport, TeamTaskStatus};

#[cfg(test)]
use crate::actor_runtime_env::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_AGENT_ID_ENV, ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
    ACTOR_RUNTIME_TEAM_ID_ENV,
};
#[cfg(test)]
#[cfg(test)]
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorInboxRequest, ActorMailboxService, ActorMessageStatus,
    ActorServiceError,
};

const TEAM_SHARED_THREAD_TITLE: &str = "all";
const TEAM_SHARED_THREAD_BOOTSTRAP_KIND: &str = "shared_thread";
const MAX_TIME_TRIGGER_DELAY_SECONDS: i64 = 30 * 24 * 60 * 60;
const TIME_TRIGGER_FUTURE_SAFETY_MARGIN_SECONDS: i64 = 1;
const ACTOR_HELP_TOPIC_INBOX: &str = "inbox";
const ACTOR_HELP_TOPIC_ACK: &str = "ack";
const ACTOR_HELP_TOPIC_SEND: &str = "send";
const ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND: &str = "permission-review-respond";
const ACTOR_HELP_TOPICS: &[&str] = &[
    "team-members",
    "team-tasks",
    "team-task-create",
    "team-task-update",
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

#[derive(Debug)]
enum ActorCommand {
    Help {
        topic: Option<&'static str>,
    },
    TeamMembers {
        team_id: Option<String>,
        run_id: Option<String>,
    },
    Inbox {
        run_id: String,
        actor_id: String,
        limit: i64,
        after_id: Option<i64>,
        include_delivered: bool,
        auto_ack: bool,
    },
    Ack {
        run_id: String,
        actor_id: String,
        message_id: i64,
    },
    TeamTasks {
        team_id: String,
        actor_id: String,
        limit: i64,
        status: Option<TeamTaskStatus>,
        include_shared_thread: bool,
    },
    TeamTaskCreate {
        team_id: String,
        actor_id: String,
        title: String,
        status: TeamTaskStatus,
        topic: Option<String>,
        context: Value,
    },
    TeamTaskUpdate {
        team_id: String,
        actor_id: String,
        task_id: String,
        status: TeamTaskStatus,
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
        permission_id: String,
        option_id: Option<String>,
        outcome: Option<String>,
    },
    Send {
        run_id: String,
        from_actor_id: String,
        to_actor_id: Option<String>,
        channel_id: Option<String>,
        channel: String,
        transport: TeamActorMessageTransport,
        route: Option<Value>,
        payload: Box<Value>,
        payload_source: ActorSendPayloadSource,
        idempotency_key: Option<String>,
    },
}
#[path = "actor_cli/execute.rs"]
mod execute;
#[path = "actor_cli/help.rs"]
mod help;
#[path = "actor_cli/output.rs"]
mod output;
#[path = "actor_cli/parse.rs"]
mod parse;
#[path = "actor_cli/runtime.rs"]
mod runtime;

use self::execute::run_actor_command;
use self::parse::parse_actor_args;

#[cfg(test)]
use self::output::{actor_output_preference_for_command, encode_actor_output};
#[cfg(test)]
use self::parse::{compute_time_trigger_fire_at, parse_actor_command};
#[cfg(test)]
use self::runtime::{
    actor_cli_internal_grpc_hint_target, actor_runtime_internal_control_requested,
    init_actor_mailbox_hint_client_from_config, init_team_manager, load_actor_inbox,
    maybe_notify_actor_new_mailbox_message_type_from_cli,
};

fn maybe_reject_legacy_actor_mcp_args(args: &[String]) -> Option<anyhow::Result<()>> {
    if args.first().map(String::as_str) == Some("actor-mcp") {
        return Some(Err(anyhow::anyhow!(
            "`agenthub actor-mcp` has been removed. Use `agenthub actor ...` instead."
        )));
    }
    None
}

pub async fn maybe_run_from_args() -> Option<anyhow::Result<()>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(result) = maybe_reject_legacy_actor_mcp_args(&args) {
        return Some(result);
    }
    if args.first().map(String::as_str) != Some("actor") {
        return None;
    }
    let parsed = parse_actor_args(&args[1..]);
    Some(match parsed {
        Ok(parsed) => run_actor_command(parsed.command, parsed.output_mode).await,
        Err(err) => Err(err),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenthub_team_actor::{
        ActorAckRequest, ActorAckResponse, ActorInboxResponse, ActorSendRequest, ActorSendResponse,
    };
    use serde::Serialize;
    use std::sync::{Arc, Mutex as StdMutex, OnceLock};
    use tokio::sync::Mutex;

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

    #[derive(Clone)]
    struct MockMailboxService {
        inbox: Vec<agenthub_team_actor::ActorMessageRecord>,
        acked_ids: Arc<StdMutex<Vec<i64>>>,
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
                message: agenthub_team_actor::ActorMessageRecord {
                    status: ActorMessageStatus::Delivered,
                    delivered_at: Some(100),
                    ..message
                },
            })
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

    fn test_internal_grpc_config(
        listen: &str,
        cert_dir: &std::path::Path,
    ) -> agenthub_config::AppConfig {
        agenthub_config::AppConfig {
            internal_grpc: Some(agenthub_config::InternalGrpcConfig {
                enabled: Some(true),
                listen: Some(listen.to_string()),
                security: Some(agenthub_config::InternalGrpcSecurityConfig {
                    mode: Some("disabled".to_string()),
                    cert_dir: Some(cert_dir.to_string_lossy().to_string()),
                }),
                auth: None,
                bootstrap: None,
            }),
            ..Default::default()
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
                assert_eq!(run_id, "run-x");
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
                if run_id == "run-x" && actor_id == "planner"
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
                if run_id == "run-y" && actor_id == "planner"
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
    fn parse_team_members_uses_env_fallback() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-members-team");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-team-members");
        }
        let args = vec!["team-members".to_string()];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers { team_id, run_id } => {
                assert_eq!(team_id.as_deref(), Some("team-members-team"));
                assert_eq!(run_id.as_deref(), Some("run-team-members"));
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
    }

    #[test]
    fn parse_team_members_accepts_run_id_flag() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-env-should-be-ignored");
            std::env::set_var(
                ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
                "run-env-should-be-ignored",
            );
        }
        let args = vec![
            "team-members".to_string(),
            "--run-id".to_string(),
            "run-explicit".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers { team_id, run_id } => {
                assert!(team_id.is_none());
                assert_eq!(run_id.as_deref(), Some("run-explicit"));
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
    }

    #[test]
    fn parse_team_members_accepts_team_id_flag_without_run() {
        let _guard = env_lock().blocking_lock();
        let prev_team = std::env::var(ACTOR_RUNTIME_TEAM_ID_ENV).ok();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_TEAM_ID_ENV, "team-env");
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-env");
        }
        let args = vec![
            "team-members".to_string(),
            "--team-id".to_string(),
            "team-explicit".to_string(),
        ];
        let parsed =
            parse_actor_command(&args, &mut ActorOutputMode::Default).expect("parse team-members");
        match parsed {
            ActorCommand::TeamMembers { team_id, run_id } => {
                assert_eq!(team_id.as_deref(), Some("team-explicit"));
                assert!(run_id.is_none());
            }
            _ => panic!("expected team-members command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_current_run);
    }

    #[test]
    fn parse_inbox_ignores_legacy_run_env_alias() {
        let _guard = env_lock().blocking_lock();
        let prev_current_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        unsafe {
            std::env::remove_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV);
            std::env::set_var("AGENTHUB_ACTOR_RUN_ID", "run-legacy-only");
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "planner");
        }
        let args = vec!["inbox".to_string()];
        let err = match parse_actor_command(&args, &mut ActorOutputMode::Default) {
            Ok(_) => panic!("legacy run env alias should be ignored"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("run_id is required"),
            "unexpected error: {err}"
        );
        unsafe {
            std::env::remove_var("AGENTHUB_ACTOR_RUN_ID");
        }
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
            ActorCommand::Send {
                idempotency_key, ..
            } => {
                let idempotency_key = idempotency_key.expect("default idempotency key");
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
            ActorCommand::Send {
                idempotency_key, ..
            } => {
                assert!(
                    idempotency_key.is_none(),
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
        assert!(err.to_string().contains("--text and --payload-json"));
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

    #[tokio::test]
    async fn actor_send_type_hint_is_best_effort_without_internal_grpc_client() {
        let _guard = env_lock().lock().await;
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV);
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV);
        }
        let (manager, config) = init_team_manager().await.expect("init team manager");
        let send_result = agenthub_team_actor::ActorSendResponse {
            message_id: 42,
            state: agenthub_team_actor::ActorMessageStatus::Pending,
            deduped: false,
            created_at: 1_700_000_000,
            message: agenthub_team_actor::ActorMessageRecord {
                message_id: 42,
                run_id: "run-cli-hint".to_string(),
                from_actor_id: "leader".to_string(),
                from_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                from_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
                to_actor_id: "worker".to_string(),
                to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                to_actor_kind: agenthub_team_actor::ActorIdentityKind::Agent,
                channel: "default".to_string(),
                transport: agenthub_team_actor::ActorMessageTransport::Local,
                route: None,
                payload: serde_json::json!({
                    "type": "worker_status",
                    "status": "ready"
                }),
                status: agenthub_team_actor::ActorMessageStatus::Pending,
                created_at: 1_700_000_000,
                delivered_at: None,
            },
        };
        maybe_notify_actor_new_mailbox_message_type_from_cli(&manager, &config, &send_result)
            .await
            .expect("best-effort mailbox hint should not fail");
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
    }

    #[tokio::test]
    async fn actor_runtime_internal_control_requested_requires_actor_and_run_env() {
        let _guard = env_lock().lock().await;
        let prev_run = std::env::var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV).ok();
        let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::remove_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV);
            std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
            std::env::set_var(
                crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
                "https://127.0.0.1:9",
            );
            std::env::set_var(
                crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
                "test-token",
            );
        }

        assert!(!actor_runtime_internal_control_requested());
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, "worker");
        }
        assert!(!actor_runtime_internal_control_requested());
        unsafe {
            std::env::set_var(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, "run-1");
        }
        assert!(actor_runtime_internal_control_requested());

        restore_env(ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, prev_run);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
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

    #[tokio::test]
    async fn init_actor_mailbox_hint_client_from_config_skips_missing_remote_token() {
        let _guard = env_lock().lock().await;
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::set_var(
                crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
                "https://127.0.0.1:50051",
            );
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV);
        }
        assert!(
            init_actor_mailbox_hint_client_from_config(&agenthub_config::AppConfig::default())
                .await
                .expect("missing token should degrade to None")
                .is_none()
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
    }

    #[tokio::test]
    async fn init_actor_mailbox_hint_client_from_config_skips_when_internal_grpc_disabled() {
        let _guard = env_lock().lock().await;
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV);
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV);
        }
        assert!(
            init_actor_mailbox_hint_client_from_config(&agenthub_config::AppConfig::default())
                .await
                .expect("disabled internal grpc should return None")
                .is_none()
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
        );
    }

    #[tokio::test]
    async fn init_actor_mailbox_hint_client_from_config_skips_invalid_listen_addr() {
        let _guard = env_lock().lock().await;
        let prev_target =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV).ok();
        let prev_token =
            std::env::var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV).ok();
        unsafe {
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV);
            std::env::remove_var(crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV);
        }
        let tempdir = std::env::temp_dir().join(format!(
            "agenthub-actor-cli-invalid-listen-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tempdir).expect("create temp cert dir");
        let config = test_internal_grpc_config("not-an-addr", &tempdir);
        assert!(
            init_actor_mailbox_hint_client_from_config(&config)
                .await
                .expect("invalid listen addr should return None")
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&tempdir);
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TARGET_ENV,
            prev_target,
        );
        restore_env(
            crate::actor_runtime_env::ACTOR_RUNTIME_INTERNAL_GRPC_TOKEN_ENV,
            prev_token,
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
        let args = vec![
            "team-members".to_string(),
            "--team-id".to_string(),
            "help".to_string(),
        ];
        let parsed = parse_actor_command(&args, &mut ActorOutputMode::Default)
            .expect("parse team-members with help value");
        match parsed {
            ActorCommand::TeamMembers { team_id, run_id } => {
                assert_eq!(team_id.as_deref(), Some("help"));
                assert!(run_id.is_none());
            }
            other => panic!("expected team-members command, got {other:?}"),
        }
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
            ActorCommand::TeamTasks {
                team_id,
                actor_id,
                status,
                include_shared_thread,
                ..
            } => {
                assert_eq!(team_id, "team-kanban");
                assert_eq!(actor_id, "leader");
                assert_eq!(status, Some(TeamTaskStatus::InReview));
                assert!(include_shared_thread);
            }
            _ => panic!("expected team-tasks command"),
        }
        restore_env(ACTOR_RUNTIME_TEAM_ID_ENV, prev_team);
        restore_env(ACTOR_RUNTIME_ACTOR_ID_ENV, prev_actor);
        restore_env(ACTOR_RUNTIME_AGENT_ID_ENV, prev_agent);
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
    fn actor_cli_internal_grpc_hint_target_forces_loopback() {
        assert_eq!(
            actor_cli_internal_grpc_hint_target("0.0.0.0:50051").as_deref(),
            Some("https://127.0.0.1:50051")
        );
        assert_eq!(
            actor_cli_internal_grpc_hint_target("127.0.0.1:50052").as_deref(),
            Some("https://127.0.0.1:50052")
        );
        assert!(actor_cli_internal_grpc_hint_target("not-an-addr").is_none());
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
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::TeamTasks {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    status: Some(TeamTaskStatus::Open),
                    limit: 10,
                    include_shared_thread: true,
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
                ActorCommand::TeamTaskUpdate {
                    team_id: "team-1".to_string(),
                    actor_id: "leader".to_string(),
                    task_id: "task-1".to_string(),
                    status: TeamTaskStatus::InProgress,
                },
                ActorOutputPreference::ToonPreferred,
            ),
            (
                ActorCommand::Inbox {
                    run_id: "run-1".to_string(),
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
                    run_id: "run-1".to_string(),
                    actor_id: "worker".to_string(),
                    message_id: 42,
                },
                ActorOutputPreference::JsonPreferred,
            ),
            (
                ActorCommand::Send {
                    run_id: "run-1".to_string(),
                    from_actor_id: "leader".to_string(),
                    to_actor_id: Some("worker".to_string()),
                    channel_id: None,
                    channel: "default".to_string(),
                    transport: TeamActorMessageTransport::Local,
                    route: None,
                    payload: Box::new(Value::String("hello".to_string())),
                    payload_source: ActorSendPayloadSource::Text,
                    idempotency_key: None,
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
                    permission_id: "perm-1".to_string(),
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
            }),
        )
        .expect("encode forced json team-members output");
        assert_eq!(output, r#"{"name":"alpha","count":2}"#);
    }
}
