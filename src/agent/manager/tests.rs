use super::{
    ACP_PROVIDER_CODEX, ACP_PROVIDER_GEMINI, ACP_PROVIDER_KIMI, ACTOR_RUNTIME_ACTOR_ID_ENV,
    ACTOR_RUNTIME_CHANNEL_ENV, ACTOR_RUNTIME_RUN_ID_ENV, AgentStatus, OutputStream,
    acp_provider_for_agent_with_binary, actor_runtime_context_from_env, expand_tilde,
    is_path_allowed, normalize_path, status_from_str, status_to_str, stream_from_str,
    stream_to_str,
};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn normalize_path_resolves_dot_and_parent() {
    assert_eq!(normalize_path("/a/b/./c"), "/a/b/c");
    assert_eq!(normalize_path("/a/b/../c"), "/a/c");
    assert_eq!(normalize_path("/a/./b/../c/."), "/a/c");
}

#[test]
fn is_path_allowed_matches_exact_or_child() {
    assert!(is_path_allowed("/home/foo", "/home/foo"));
    assert!(is_path_allowed("/home/foo/bar", "/home/foo"));
    assert!(is_path_allowed("/home/foo/bar/baz", "/home/foo/bar"));
    assert!(!is_path_allowed("/home/foobar", "/home/foo"));
    assert!(!is_path_allowed("/home/foo/../bar", "/home/foo"));
}

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
}

#[test]
fn actor_runtime_context_uses_env_and_agent_defaults() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev_run = std::env::var(ACTOR_RUNTIME_RUN_ID_ENV).ok();
    let prev_actor = std::env::var(ACTOR_RUNTIME_ACTOR_ID_ENV).ok();
    let prev_channel = std::env::var(ACTOR_RUNTIME_CHANNEL_ENV).ok();

    unsafe {
        std::env::set_var(ACTOR_RUNTIME_RUN_ID_ENV, "run-42");
        std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV);
        std::env::set_var(ACTOR_RUNTIME_CHANNEL_ENV, "coordination");
    }

    let context = actor_runtime_context_from_env("planner-agent").expect("actor context");
    assert_eq!(context.run_id, "run-42");
    assert_eq!(context.actor_id, "planner-agent");
    assert_eq!(context.default_channel, "coordination");
    assert!(!context.actor_cli_path.trim().is_empty());

    if let Some(value) = prev_run {
        unsafe { std::env::set_var(ACTOR_RUNTIME_RUN_ID_ENV, value) }
    } else {
        unsafe { std::env::remove_var(ACTOR_RUNTIME_RUN_ID_ENV) }
    }
    if let Some(value) = prev_actor {
        unsafe { std::env::set_var(ACTOR_RUNTIME_ACTOR_ID_ENV, value) }
    } else {
        unsafe { std::env::remove_var(ACTOR_RUNTIME_ACTOR_ID_ENV) }
    }
    if let Some(value) = prev_channel {
        unsafe { std::env::set_var(ACTOR_RUNTIME_CHANNEL_ENV, value) }
    } else {
        unsafe { std::env::remove_var(ACTOR_RUNTIME_CHANNEL_ENV) }
    }
}

#[test]
fn actor_runtime_context_requires_run_id() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev_run = std::env::var(ACTOR_RUNTIME_RUN_ID_ENV).ok();
    unsafe {
        std::env::remove_var(ACTOR_RUNTIME_RUN_ID_ENV);
    }

    assert!(
        actor_runtime_context_from_env("planner-agent").is_none(),
        "actor context should be disabled without run id"
    );

    if let Some(value) = prev_run {
        unsafe { std::env::set_var(ACTOR_RUNTIME_RUN_ID_ENV, value) }
    } else {
        unsafe { std::env::remove_var(ACTOR_RUNTIME_RUN_ID_ENV) }
    }
}
