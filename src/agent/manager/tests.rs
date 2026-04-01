use super::acp_provider::{
    ACP_PROVIDER_CODEX, ACP_PROVIDER_GEMINI, ACP_PROVIDER_KIMI, AcpDefaultModeBehavior,
    acp_provider_for_agent_with_binary, acp_provider_spec_for_agent_with_binary,
};
use super::codec::{is_acp_message, status_from_str, stream_to_str};
use super::start_plan::{AgentStartPlan, build_agent_start_plan};
use super::{
    AgentRecord, AgentStatus, OutputStream, WorktreeMode, build_runtime_start_policy,
    ensure_team_leader_workdir_exists, status_to_str, stream_from_str,
};
use crate::acp::{
    AcpActorSkillContext, AcpPromptDeliveryPolicy, AcpRuntimeLocation, default_actor_cli_path,
};
use crate::path_utils::expand_tilde;
use std::sync::Mutex;
use uuid::Uuid;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn expand_tilde_uses_home() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", "/tmp/test-home");
    }
    assert_eq!(expand_tilde("~"), "/tmp/test-home");
    assert_eq!(expand_tilde("~/work"), "/tmp/test-home/work");
    if let Some(val) = prev {
        unsafe {
            std::env::set_var("HOME", val);
        }
    } else {
        unsafe {
            std::env::remove_var("HOME");
        }
    }
}

#[test]
fn status_roundtrip() {
    let statuses = [
        AgentStatus::Created,
        AgentStatus::Running,
        AgentStatus::Stopped,
        AgentStatus::Exited,
        AgentStatus::Failed,
    ];
    for status in statuses {
        let s = status_to_str(&status);
        let parsed = status_from_str(s);
        assert_eq!(status, parsed);
    }
}

#[test]
fn stream_roundtrip() {
    let streams = [
        OutputStream::Stdout,
        OutputStream::Stderr,
        OutputStream::System,
        OutputStream::Acp,
    ];
    for stream in streams {
        let s = stream_to_str(&stream);
        let parsed = stream_from_str(s);
        assert_eq!(stream, parsed);
    }
}

#[test]
fn acp_provider_for_agent_requires_expected_args() {
    let codex_bin = "agenthub-codex-acp";
    assert_eq!(
        acp_provider_for_agent_with_binary(codex_bin, "gemini", &[]),
        None
    );
    assert_eq!(
        acp_provider_for_agent_with_binary(codex_bin, "gemini", &["--acp".to_string()]),
        Some(ACP_PROVIDER_GEMINI)
    );
    assert_eq!(
        acp_provider_for_agent_with_binary(
            codex_bin,
            "gemini",
            &["--experimental-acp".to_string()]
        ),
        Some(ACP_PROVIDER_GEMINI)
    );
    assert_eq!(
        acp_provider_for_agent_with_binary(codex_bin, "kimi", &[]),
        None
    );
    assert_eq!(
        acp_provider_for_agent_with_binary(codex_bin, "kimi", &["acp".to_string()]),
        Some(ACP_PROVIDER_KIMI)
    );
    assert_eq!(
        acp_provider_for_agent_with_binary(codex_bin, "codex-acp", &[]),
        Some(ACP_PROVIDER_CODEX)
    );
    assert_eq!(
        acp_provider_for_agent_with_binary(codex_bin, codex_bin, &[]),
        Some(ACP_PROVIDER_CODEX)
    );

    let gemini =
        acp_provider_spec_for_agent_with_binary(codex_bin, "gemini", &["--acp".to_string()])
            .expect("resolve gemini acp provider");
    assert_eq!(gemini.id, ACP_PROVIDER_GEMINI);
    assert_eq!(
        gemini.prompt_delivery_policy,
        AcpPromptDeliveryPolicy::StrictFifo
    );
    assert_eq!(
        gemini.default_mode_behavior,
        AcpDefaultModeBehavior::IgnoreConfigured
    );

    let kimi = acp_provider_spec_for_agent_with_binary(codex_bin, "kimi", &["acp".to_string()])
        .expect("resolve kimi acp provider");
    assert_eq!(kimi.id, ACP_PROVIDER_KIMI);
    assert_eq!(
        kimi.prompt_delivery_policy,
        AcpPromptDeliveryPolicy::StrictFifo
    );
    assert_eq!(
        kimi.default_mode_behavior,
        AcpDefaultModeBehavior::IgnoreConfigured
    );

    let codex = acp_provider_spec_for_agent_with_binary(codex_bin, codex_bin, &[])
        .expect("resolve codex acp provider");
    assert_eq!(codex.id, ACP_PROVIDER_CODEX);
    assert_eq!(
        codex.prompt_delivery_policy,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    );
    assert_eq!(
        codex.default_mode_behavior,
        AcpDefaultModeBehavior::ApplyWhenConfigured
    );
}

#[test]
fn runtime_location_defaults_to_local_process() {
    assert_eq!(
        AcpRuntimeLocation::default(),
        AcpRuntimeLocation::LocalProcess
    );
}

#[test]
fn build_agent_start_plan_reuses_running_local_session() {
    let agent = build_agent_record_for_policy(WorktreeMode::UseExisting, "/tmp/agent", None);
    let plan = build_agent_start_plan(agent, None, Some("session-1")).expect("build start plan");
    match plan {
        AgentStartPlan::ReuseRunningSession { session_id } => {
            assert_eq!(session_id, "session-1");
        }
        other => panic!("expected running-session reuse, got {other:?}"),
    }
}

#[test]
fn build_agent_start_plan_requires_idle_session_for_new_actor_context() {
    let agent = build_agent_record_for_policy(WorktreeMode::UseExisting, "/tmp/agent", None);
    let actor_context = AcpActorSkillContext {
        team_id: Some("team-1".to_string()),
        current_run_id: Some("run-1".to_string()),
        actor_id: "leader-1".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: "/tmp/agenthub".to_string(),
        member_role: Some("leader".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };
    let err = build_agent_start_plan(agent, Some(actor_context), Some("session-1"))
        .expect_err("reject new actor context for running local agent");
    assert!(
        err.to_string()
            .contains("agent already running with session 'session-1'"),
        "unexpected error: {err}"
    );
}

#[test]
fn build_agent_start_plan_prioritizes_remote_target_over_local_reuse() {
    let mut agent = build_agent_record_for_policy(WorktreeMode::UseExisting, "/tmp/agent", None);
    agent.target_node_id = Some("node-east".to_string());

    let plan = build_agent_start_plan(agent, None, Some("session-1")).expect("build start plan");
    match plan {
        AgentStartPlan::StartRemote {
            target_node_id,
            actor_context,
            ..
        } => {
            assert_eq!(target_node_id, "node-east");
            assert!(actor_context.is_none());
        }
        other => panic!("expected remote start plan, got {other:?}"),
    }
}

#[test]
fn default_actor_cli_path_returns_non_empty_value() {
    let path = default_actor_cli_path().expect("resolve default actor cli path");
    assert!(!path.trim().is_empty());
}

#[test]
fn is_acp_message_accepts_latest_codex_event_types() {
    assert!(is_acp_message(r#"{"type":"agent_message","message":"ok"}"#));
    assert!(is_acp_message(
        r#"{"type":"plan","steps":[{"title":"Investigate"}]}"#
    ));
    assert!(is_acp_message(
        r#"{"type":"available_commands","commands":["/compact","/undo"]}"#
    ));
    assert!(is_acp_message(
        r#"{"type":"current_mode","current_mode_id":"code"}"#
    ));
    assert!(is_acp_message(
        r#"{"type":"run_status","status":"completed","session_id":"s-1"}"#
    ));
}

#[test]
fn is_acp_message_rejects_non_acp_shapes() {
    assert!(!is_acp_message("plain text"));
    assert!(!is_acp_message(r#"{"message":"missing type"}"#));
    assert!(!is_acp_message(r#"{"type":123}"#));
    assert!(!is_acp_message(r#"{"type":"   "}"#));
}

fn build_agent_record_for_policy(
    worktree_mode: WorktreeMode,
    workdir: &str,
    worktree_repo: Option<&str>,
) -> AgentRecord {
    AgentRecord {
        id: "agent-policy".to_string(),
        name: "agent-policy".to_string(),
        workdir: workdir.to_string(),
        command: "env".to_string(),
        args: vec![],
        target_node_id: None,
        worktree_mode,
        worktree_repo: worktree_repo.map(str::to_string),
        worktree_ref: None,
        code_mode: true,
        agent_loop_enabled: false,
        agent_loop_idle_seconds: None,
        agent_loop_prompt: None,
        status: AgentStatus::Created,
        created_at: 0,
        updated_at: 0,
    }
}

#[test]
fn runtime_start_policy_redirects_non_empty_leader_workdir_to_session_sandbox() {
    let tmp = std::env::temp_dir().join(format!("agenthub-leader-policy-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("create temp dir");
    std::fs::write(tmp.join("README.md"), "busy").expect("write temp marker");
    let agent =
        build_agent_record_for_policy(WorktreeMode::UseExisting, &tmp.to_string_lossy(), None);
    let ctx = AcpActorSkillContext {
        team_id: Some("team-leader".to_string()),
        current_run_id: Some("run-leader".to_string()),
        actor_id: "leader-1".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: "/tmp/agenthub".to_string(),
        member_role: Some("leader".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };
    let session_id = "session-leader-1";

    let policy =
        build_runtime_start_policy(&agent, Some(&ctx), &agent.workdir, None, Some(session_id))
            .expect("leader should derive a session-isolated workdir");
    assert!(
        policy.workdir.starts_with(&agent.workdir),
        "workdir={} base={}",
        policy.workdir,
        agent.workdir
    );
    assert!(
        policy
            .workdir
            .contains(".agenthub-team-leader/leader-1-run-leader-session-leader-1"),
        "workdir={}",
        policy.workdir
    );
}

#[test]
fn runtime_start_policy_allows_worker_use_existing_workdir_for_validation() {
    let tmp = std::env::temp_dir().join(format!("agenthub-worker-policy-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("create temp dir");
    let agent =
        build_agent_record_for_policy(WorktreeMode::UseExisting, &tmp.to_string_lossy(), None);
    let ctx = AcpActorSkillContext {
        team_id: Some("team-worker".to_string()),
        current_run_id: Some("run-worker".to_string()),
        actor_id: "worker-1".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: "/tmp/agenthub".to_string(),
        member_role: Some("worker".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let policy = build_runtime_start_policy(&agent, Some(&ctx), &agent.workdir, None, None)
        .expect("worker use_existing workdir should be allowed");
    assert!(matches!(policy.worktree_mode, WorktreeMode::UseExisting));
    assert_eq!(policy.workdir, agent.workdir);
    assert!(policy.worktree_repo.is_none());
    assert!(policy.worker_branch.is_none());
}

#[test]
fn runtime_start_policy_assigns_worker_run_isolated_worktree_and_branch() {
    let tmp_root = std::env::temp_dir().join(format!("agenthub-worker-policy-{}", Uuid::new_v4()));
    let workdir = tmp_root.join("worker-base");
    let repo = tmp_root.join("repo");
    std::fs::create_dir_all(&workdir).expect("create worker workdir");
    std::fs::create_dir_all(&repo).expect("create repo dir");
    let workdir_str = workdir.to_string_lossy().to_string();
    let repo_str = repo.to_string_lossy().to_string();
    let agent =
        build_agent_record_for_policy(WorktreeMode::CreateWorktree, &workdir_str, Some(&repo_str));
    let ctx = AcpActorSkillContext {
        team_id: Some("team-worker".to_string()),
        current_run_id: Some("run-1234-5678".to_string()),
        actor_id: "worker-alpha".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: "/tmp/agenthub".to_string(),
        member_role: Some("worker".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let policy =
        build_runtime_start_policy(&agent, Some(&ctx), &workdir_str, Some(&repo_str), None)
            .expect("build worker runtime policy");
    assert!(matches!(policy.worktree_mode, WorktreeMode::CreateWorktree));
    assert_eq!(policy.worktree_ref.as_deref(), Some("HEAD"));
    assert_eq!(policy.worktree_repo.as_deref(), Some(repo_str.as_str()));
    assert!(
        policy.workdir.contains("worker-alpha-run-1234-5678"),
        "workdir={}",
        policy.workdir
    );
    let branch = policy.worker_branch.as_deref().unwrap_or_default();
    assert!(
        branch.starts_with("worker-worker-alpha-"),
        "branch={branch}"
    );
}

#[test]
fn runtime_start_policy_rejects_non_empty_leader_workdir_without_session_id() {
    let tmp = std::env::temp_dir().join(format!("agenthub-leader-policy-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("create temp dir");
    std::fs::write(tmp.join("README.md"), "busy").expect("write temp marker");
    let agent =
        build_agent_record_for_policy(WorktreeMode::UseExisting, &tmp.to_string_lossy(), None);
    let ctx = AcpActorSkillContext {
        team_id: Some("team-leader".to_string()),
        current_run_id: Some("run-leader".to_string()),
        actor_id: "leader-1".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: "/tmp/agenthub".to_string(),
        member_role: Some("leader".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let err = build_runtime_start_policy(&agent, Some(&ctx), &agent.workdir, None, None)
        .expect_err("leader should require a start session id for non-empty workdir");
    assert!(
        err.to_string()
            .contains("leader role policy requires start session id")
    );
}

#[test]
fn ensure_team_leader_workdir_exists_creates_missing_leader_dir() {
    let path = std::env::temp_dir().join(format!("agenthub-leader-workdir-{}", Uuid::new_v4()));
    let workdir = path.to_string_lossy().to_string();
    let ctx = AcpActorSkillContext {
        team_id: Some("team-leader".to_string()),
        current_run_id: Some("run-leader".to_string()),
        actor_id: "leader-1".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: "/tmp/agenthub".to_string(),
        member_role: Some("leader".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    assert!(!path.exists(), "temp path should not exist before test");
    ensure_team_leader_workdir_exists(Some(&ctx), &workdir).expect("create leader workdir");
    assert!(path.exists(), "leader workdir should be created");
    assert!(path.is_dir(), "leader workdir should be a directory");
}

#[test]
fn ensure_team_leader_workdir_exists_ignores_non_leader_context() {
    let path = std::env::temp_dir().join(format!("agenthub-non-leader-workdir-{}", Uuid::new_v4()));
    let workdir = path.to_string_lossy().to_string();
    let ctx = AcpActorSkillContext {
        team_id: Some("team-worker".to_string()),
        current_run_id: Some("run-worker".to_string()),
        actor_id: "worker-1".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: "/tmp/agenthub".to_string(),
        member_role: Some("worker".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    assert!(!path.exists(), "temp path should not exist before test");
    ensure_team_leader_workdir_exists(Some(&ctx), &workdir)
        .expect("non-leader should not require directory creation");
    assert!(
        !path.exists(),
        "non-leader helper path should not be created automatically"
    );
}

#[test]
fn ensure_team_leader_workdir_exists_reports_creation_error() {
    let root = std::env::temp_dir().join(format!("agenthub-leader-file-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    let file_path = root.join("not-a-dir");
    std::fs::write(&file_path, "marker").expect("create temp file");
    let impossible_dir = file_path.join("child");
    let workdir = impossible_dir.to_string_lossy().to_string();
    let ctx = AcpActorSkillContext {
        team_id: Some("team-leader".to_string()),
        current_run_id: Some("run-leader".to_string()),
        actor_id: "leader-2".to_string(),
        default_channel: "default".to_string(),
        actor_cli_path: "/tmp/agenthub".to_string(),
        member_role: Some("leader".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let err = ensure_team_leader_workdir_exists(Some(&ctx), &workdir)
        .expect_err("invalid leader path should fail directory creation");
    assert!(
        err.to_string().contains("failed to create leader workdir"),
        "err={err}"
    );
}
