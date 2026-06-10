use super::acp_provider::{
    ACP_PROVIDER_CLAUDE, ACP_PROVIDER_CODEX, ACP_PROVIDER_GEMINI, ACP_PROVIDER_KIMI,
    AcpDefaultModeBehavior, acp_provider_for_agent_with_binary,
    acp_provider_spec_for_agent_with_binary,
};
use super::codec::{is_acp_message, status_from_str, stream_to_str};
use super::session::effective_acp_default_mode;
use super::start_plan::{AgentStartPlan, build_agent_start_plan};
use super::{
    AGENT_LOOP_MESSAGE_ID_PREFIX, AgentOutput, AgentRecord, AgentStatus, OutputStream,
    WorktreeMode, acp_accepts_best_effort_hint, build_runtime_start_policy,
    ensure_team_runtime_workspace_layout, is_agent_loop_activity_output,
    safe_acp_provider_diagnostics_details, should_rearm_agent_loop_for_output, status_to_str,
    stream_from_str,
};
use crate::acp::{
    AcpActorSkillContext, AcpCommandErrorDiagnostic, AcpHandleDiagnostics, AcpPromptDeliveryPolicy,
    AcpRuntimeLocation, AcpStalePromptDiagnostic, AcpToolCallDiagnostic,
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
fn agent_loop_activity_counts_non_loop_acp_output_only() {
    let base = AgentOutput {
        event_id: 1,
        agent_id: "agent-1".to_string(),
        session_id: "session-1".to_string(),
        seq: "seq-1".to_string(),
        ts: 1,
        stream: OutputStream::Acp,
        message: r#"{"type":"agent_message","message":"working"}"#.to_string(),
    };
    assert!(is_agent_loop_activity_output(&base));

    let human_input = AgentOutput {
        message: r#"{"type":"user_message","text":"please continue","message_id":"human-1"}"#
            .to_string(),
        ..base.clone()
    };
    assert!(
        is_agent_loop_activity_output(&human_input),
        "human ACP user_message should rearm the loop"
    );

    let loop_input = AgentOutput {
        message: format!(
            r#"{{"type":"user_message","text":"loop prompt","message_id":"{AGENT_LOOP_MESSAGE_ID_PREFIX}seq-2"}}"#
        ),
        ..base.clone()
    };
    assert!(
        !is_agent_loop_activity_output(&loop_input),
        "synthetic loop prompt must not rearm the loop"
    );

    let system_output = AgentOutput {
        stream: OutputStream::System,
        message: "process exited".to_string(),
        ..base
    };
    assert!(
        !is_agent_loop_activity_output(&system_output),
        "non-ACP output should not reset ACP silence tracking"
    );
}

fn idle_acp_hint_diagnostics() -> AcpHandleDiagnostics {
    AcpHandleDiagnostics {
        session_id: "session-1".to_string(),
        command_channel_closed: false,
        command_channel_capacity: 8,
        command_channel_max_capacity: 8,
        active_prompt_count: 0,
        pending_command_count: 0,
        pending_permission_count: 0,
        active_submission_ids: Vec::new(),
        last_submission_id: None,
        last_provider_event_type: None,
        last_provider_event_at: None,
        pending_tool_call_count: 0,
        pending_tool_calls: Vec::new(),
        stale_prompt: None,
        last_command_error: None,
        last_command_error_at: None,
    }
}

#[test]
fn best_effort_mailbox_hints_respect_provider_prompt_policy() {
    assert!(acp_accepts_best_effort_hint(
        &idle_acp_hint_diagnostics(),
        AcpPromptDeliveryPolicy::StrictFifo
    ));

    let mut active_prompt = idle_acp_hint_diagnostics();
    active_prompt.active_prompt_count = 1;
    assert!(!acp_accepts_best_effort_hint(
        &active_prompt,
        AcpPromptDeliveryPolicy::StrictFifo
    ));
    assert!(acp_accepts_best_effort_hint(
        &active_prompt,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    ));

    let mut queued_command = idle_acp_hint_diagnostics();
    queued_command.pending_command_count = 1;
    assert!(!acp_accepts_best_effort_hint(
        &queued_command,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    ));

    let mut pending_permission = idle_acp_hint_diagnostics();
    pending_permission.pending_permission_count = 1;
    assert!(!acp_accepts_best_effort_hint(
        &pending_permission,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    ));

    let mut pending_tool = idle_acp_hint_diagnostics();
    pending_tool.pending_tool_call_count = 1;
    assert!(!acp_accepts_best_effort_hint(
        &pending_tool,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    ));

    let mut stale_prompt = idle_acp_hint_diagnostics();
    stale_prompt.stale_prompt = Some(AcpStalePromptDiagnostic {
        active_prompt_count: 1,
        pending_permission_count: 0,
        stale_for_seconds: 300,
        last_activity_at: Some(100),
        active_submission_ids: vec!["submission-1".to_string()],
    });
    assert!(!acp_accepts_best_effort_hint(
        &stale_prompt,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    ));

    let mut closed_channel = idle_acp_hint_diagnostics();
    closed_channel.command_channel_closed = true;
    assert!(!acp_accepts_best_effort_hint(
        &closed_channel,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    ));

    let mut full_channel = idle_acp_hint_diagnostics();
    full_channel.command_channel_capacity = 0;
    assert!(!acp_accepts_best_effort_hint(
        &full_channel,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    ));

    let mut queued_channel_send = idle_acp_hint_diagnostics();
    queued_channel_send.command_channel_capacity = 7;
    assert!(acp_accepts_best_effort_hint(
        &queued_channel_send,
        AcpPromptDeliveryPolicy::AllowConcurrentPrompts
    ));

    let mut previous_error = idle_acp_hint_diagnostics();
    previous_error.last_command_error = Some(AcpCommandErrorDiagnostic {
        command_kind: "prompt".to_string(),
        message: "previous transient error".to_string(),
    });
    assert!(acp_accepts_best_effort_hint(
        &previous_error,
        AcpPromptDeliveryPolicy::StrictFifo
    ));
}

#[test]
fn provider_diagnostics_details_redact_error_messages_and_keep_safe_ids() {
    let mut diagnostics = idle_acp_hint_diagnostics();
    diagnostics.active_submission_ids = vec!["submission-1".to_string()];
    diagnostics.last_submission_id = Some("submission-0".to_string());
    diagnostics.last_provider_event_type = Some("turn_started".to_string());
    diagnostics.pending_tool_calls = vec![AcpToolCallDiagnostic {
        tool_call_id: "tool-1".to_string(),
        status: "running".to_string(),
        updated_at: Some(123),
    }];
    diagnostics.pending_tool_call_count = diagnostics.pending_tool_calls.len();
    diagnostics.stale_prompt = Some(AcpStalePromptDiagnostic {
        active_prompt_count: 1,
        pending_permission_count: 0,
        stale_for_seconds: 300,
        last_activity_at: Some(100),
        active_submission_ids: vec!["submission-1".to_string()],
    });
    diagnostics.last_command_error = Some(AcpCommandErrorDiagnostic {
        command_kind: "prompt".to_string(),
        message: "prompt body and tool arguments must stay private".to_string(),
    });

    let details = safe_acp_provider_diagnostics_details(&diagnostics);

    assert_eq!(details["session_id"], "session-1");
    assert_eq!(details["active_submission_ids"][0], "submission-1");
    assert_eq!(details["pending_tool_calls"][0]["tool_call_id"], "tool-1");
    assert_eq!(details["last_command_error"]["command_kind"], "prompt");
    assert!(details["last_command_error"].get("message").is_none());
    assert!(
        !details
            .to_string()
            .contains("prompt body and tool arguments must stay private")
    );
}

#[test]
fn agent_loop_rearm_requires_same_session_and_real_acp_activity() {
    let base = AgentOutput {
        event_id: 1,
        agent_id: "agent-1".to_string(),
        session_id: "session-1".to_string(),
        seq: "seq-1".to_string(),
        ts: 1,
        stream: OutputStream::Acp,
        message: r#"{"type":"agent_message","message":"working"}"#.to_string(),
    };

    assert!(
        should_rearm_agent_loop_for_output("session-1", &base),
        "matching-session ACP activity should rearm"
    );

    let other_session = AgentOutput {
        session_id: "session-2".to_string(),
        ..base.clone()
    };
    assert!(
        !should_rearm_agent_loop_for_output("session-1", &other_session),
        "other-session activity must not rearm"
    );

    let synthetic_loop = AgentOutput {
        message: format!(
            r#"{{"type":"user_message","text":"loop prompt","message_id":"{AGENT_LOOP_MESSAGE_ID_PREFIX}seq-2"}}"#
        ),
        ..base.clone()
    };
    assert!(
        !should_rearm_agent_loop_for_output("session-1", &synthetic_loop),
        "synthetic loop prompt must not rearm"
    );

    let non_acp = AgentOutput {
        stream: OutputStream::System,
        message: "process exited".to_string(),
        ..base
    };
    assert!(
        !should_rearm_agent_loop_for_output("session-1", &non_acp),
        "non-ACP output must not rearm"
    );
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
        acp_provider_for_agent_with_binary(codex_bin, "claude-agent-acp", &[]),
        Some(ACP_PROVIDER_CLAUDE)
    );
    assert_eq!(
        acp_provider_for_agent_with_binary(codex_bin, "claude-code-acp-rs", &[]),
        None
    );
    assert_eq!(
        acp_provider_for_agent_with_binary(codex_bin, "claude-code-acp-rs", &["--acp".to_string()]),
        Some(ACP_PROVIDER_CLAUDE)
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

    let claude = acp_provider_spec_for_agent_with_binary(codex_bin, "claude-agent-acp", &[])
        .expect("resolve claude acp provider");
    assert_eq!(claude.id, ACP_PROVIDER_CLAUDE);
    assert_eq!(
        claude.prompt_delivery_policy,
        AcpPromptDeliveryPolicy::StrictFifo
    );
    assert_eq!(
        claude.default_mode_behavior,
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
fn team_codex_acp_default_mode_uses_full_access() {
    let codex_bin = "agenthub-codex-acp";
    let codex = acp_provider_spec_for_agent_with_binary(codex_bin, codex_bin, &[])
        .expect("resolve codex acp provider");
    let gemini =
        acp_provider_spec_for_agent_with_binary(codex_bin, "gemini", &["--acp".to_string()])
            .expect("resolve gemini acp provider");

    assert_eq!(
        effective_acp_default_mode(codex, None, Some("auto"), true),
        Some("full-access")
    );
    assert_eq!(
        effective_acp_default_mode(codex, Some("read-only"), Some("auto"), true),
        Some("read-only")
    );
    assert_eq!(
        effective_acp_default_mode(codex, Some("read-only"), Some("auto"), false),
        Some("read-only")
    );
    assert_eq!(
        effective_acp_default_mode(codex, None, Some("auto"), false),
        Some("auto")
    );
    assert_eq!(
        effective_acp_default_mode(gemini, None, Some("auto"), true),
        Some("auto")
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
        actor_id: "coordinator-1".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("coordinator".to_string()),
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
        codex_acp_default_mode: None,
        agent_loop_enabled: false,
        agent_loop_idle_seconds: None,
        agent_loop_prompt: None,
        status: AgentStatus::Created,
        created_at: 0,
        updated_at: 0,
    }
}

#[test]
fn runtime_start_policy_redirects_coordinator_to_stable_sandbox() {
    let tmp = std::env::temp_dir().join(format!("agenthub-coordinator-policy-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("create temp dir");
    std::fs::write(tmp.join("README.md"), "busy").expect("write temp marker");
    let agent =
        build_agent_record_for_policy(WorktreeMode::UseExisting, &tmp.to_string_lossy(), None);
    let ctx = AcpActorSkillContext {
        team_id: Some("team-coordinator".to_string()),
        current_run_id: Some("run-coordinator".to_string()),
        actor_id: "coordinator-1".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("coordinator".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let policy = build_runtime_start_policy(&agent, Some(&ctx), &agent.workdir, None, None)
        .expect("coordinator should derive a stable coordination workdir");
    assert!(
        policy.workdir.starts_with(&agent.workdir),
        "workdir={} base={}",
        policy.workdir,
        agent.workdir
    );
    assert!(
        policy
            .workdir
            .contains(".agenthub-team-coordinator/coordinator-1-run-coordinator"),
        "workdir={}",
        policy.workdir
    );
    assert!(
        !policy.workdir.contains("session-coordinator"),
        "coordinator sandbox should not depend on launch session id: workdir={}",
        policy.workdir
    );
}

#[test]
fn runtime_start_policy_reuses_legacy_leader_coordination_workdir_when_present() {
    let tmp = std::env::temp_dir().join(format!(
        "agenthub-coordinator-legacy-policy-{}",
        Uuid::new_v4()
    ));
    let legacy_dir = tmp.join(".agenthub-team-leader/coordinator-1-run-coordinator");
    std::fs::create_dir_all(&legacy_dir).expect("create legacy coordinator dir");
    let agent =
        build_agent_record_for_policy(WorktreeMode::UseExisting, &tmp.to_string_lossy(), None);
    let ctx = AcpActorSkillContext {
        team_id: Some("team-coordinator".to_string()),
        current_run_id: Some("run-coordinator".to_string()),
        actor_id: "coordinator-1".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("coordinator".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let policy = build_runtime_start_policy(&agent, Some(&ctx), &agent.workdir, None, None)
        .expect("coordinator should reuse legacy coordination workdir");
    assert_eq!(policy.workdir, legacy_dir.to_string_lossy());
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
fn runtime_start_policy_reuses_coordinator_workspace_across_launch_ids() {
    let tmp = std::env::temp_dir().join(format!("agenthub-coordinator-policy-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&tmp).expect("create temp dir");
    let agent =
        build_agent_record_for_policy(WorktreeMode::UseExisting, &tmp.to_string_lossy(), None);
    let ctx = AcpActorSkillContext {
        team_id: Some("team-coordinator".to_string()),
        current_run_id: Some("run-coordinator".to_string()),
        actor_id: "coordinator-1".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("coordinator".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let first = build_runtime_start_policy(
        &agent,
        Some(&ctx),
        &agent.workdir,
        None,
        Some("session-coordinator-1"),
    )
    .expect("first coordinator launch policy");
    let second = build_runtime_start_policy(
        &agent,
        Some(&ctx),
        &agent.workdir,
        None,
        Some("session-coordinator-2"),
    )
    .expect("second coordinator launch policy");
    assert_eq!(first.workdir, second.workdir);
}

#[tokio::test]
async fn ensure_team_runtime_workspace_layout_creates_missing_coordinator_dir() {
    let path =
        std::env::temp_dir().join(format!("agenthub-coordinator-workdir-{}", Uuid::new_v4()));
    let workdir = path.to_string_lossy().to_string();
    let ctx = AcpActorSkillContext {
        team_id: Some("team-coordinator".to_string()),
        current_run_id: Some("run-coordinator".to_string()),
        actor_id: "coordinator-1".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("coordinator".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    assert!(!path.exists(), "temp path should not exist before test");
    ensure_team_runtime_workspace_layout(Some(&ctx), &workdir)
        .await
        .expect("create coordinator runtime workspace");
    assert!(path.exists(), "coordinator workdir should be created");
    assert!(path.is_dir(), "coordinator workdir should be a directory");
    assert!(
        path.join(".cache/context/run").is_dir(),
        "coordinator context run dir should exist"
    );
    assert!(
        path.join(".cache/context/memory").is_dir(),
        "coordinator context memory dir should exist"
    );
    for relative_path in [
        ".cache/context/state.md",
        ".cache/context/decisions.md",
        ".cache/context/errors.md",
        ".cache/context/log.md",
        ".cache/context/memory/profile.md",
        ".cache/context/memory/project_facts.md",
        ".cache/context/memory/decision_journal.md",
        ".cache/context/memory/open_questions.md",
    ] {
        assert!(
            path.join(relative_path).is_file(),
            "coordinator context file should exist: {relative_path}"
        );
    }
}

#[tokio::test]
async fn ensure_team_runtime_workspace_layout_initializes_worker_context_in_existing_workdir() {
    let path = std::env::temp_dir().join(format!("agenthub-worker-workdir-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).expect("create temp workdir");
    let workdir = path.to_string_lossy().to_string();
    let ctx = AcpActorSkillContext {
        team_id: Some("team-worker".to_string()),
        current_run_id: Some("run-worker".to_string()),
        actor_id: "worker-1".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("worker".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    ensure_team_runtime_workspace_layout(Some(&ctx), &workdir)
        .await
        .expect("worker runtime workspace should initialize context layout");
    assert!(
        path.join(".cache/context/run").is_dir(),
        "worker context run dir should exist"
    );
    assert!(
        path.join(".cache/context/memory").is_dir(),
        "worker context memory dir should exist"
    );
}

#[tokio::test]
async fn ensure_team_runtime_workspace_layout_ignores_non_team_context() {
    let path = std::env::temp_dir().join(format!(
        "agenthub-non-coordinator-workdir-{}",
        Uuid::new_v4()
    ));
    let workdir = path.to_string_lossy().to_string();
    let ctx = AcpActorSkillContext {
        team_id: Some("team-worker".to_string()),
        current_run_id: Some("run-worker".to_string()),
        actor_id: "worker-1".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("worker".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    assert!(!path.exists(), "temp path should not exist before test");
    ensure_team_runtime_workspace_layout(Some(&ctx), &workdir)
        .await
        .expect("non-team context should not require directory creation");
    assert!(
        !path.exists(),
        "non-team helper path should not be created automatically"
    );
}

#[tokio::test]
async fn ensure_team_runtime_workspace_layout_reports_creation_error() {
    let root = std::env::temp_dir().join(format!("agenthub-coordinator-file-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    let file_path = root.join("not-a-dir");
    std::fs::write(&file_path, "marker").expect("create temp file");
    let impossible_dir = file_path.join("child");
    let workdir = impossible_dir.to_string_lossy().to_string();
    let ctx = AcpActorSkillContext {
        team_id: Some("team-coordinator".to_string()),
        current_run_id: Some("run-coordinator".to_string()),
        actor_id: "coordinator-2".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("coordinator".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let err = ensure_team_runtime_workspace_layout(Some(&ctx), &workdir)
        .await
        .expect_err("invalid coordinator path should fail directory creation");
    assert!(
        err.to_string()
            .contains("failed to stat team runtime workdir"),
        "err={err}"
    );
}

#[tokio::test]
async fn ensure_team_runtime_workspace_layout_reports_non_file_context_entries() {
    let path = std::env::temp_dir().join(format!(
        "agenthub-coordinator-workdir-conflict-{}",
        Uuid::new_v4()
    ));
    let conflicting_file = path.join(".cache/context/state.md");
    std::fs::create_dir_all(&conflicting_file).expect("create conflicting directory");
    let workdir = path.to_string_lossy().to_string();
    let ctx = AcpActorSkillContext {
        team_id: Some("team-coordinator".to_string()),
        current_run_id: Some("run-coordinator".to_string()),
        actor_id: "coordinator-3".to_string(),
        default_channel: "default".to_string(),
        member_role: Some("coordinator".to_string()),
        member_skills: Vec::new(),
        contract_version: None,
        continuity: None,
    };

    let err = ensure_team_runtime_workspace_layout(Some(&ctx), &workdir)
        .await
        .expect_err("directory at context file path should fail");
    assert!(
        err.to_string()
            .contains("team runtime context path is not a file"),
        "err={err}"
    );
}
