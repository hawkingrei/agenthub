use std::collections::HashMap;

use super::*;
use agenthub_config::{NowledgeMemConfig, NowledgeMemProfileConfig, NowledgeMemTeamBindingConfig};

pub(super) fn config() -> AppConfig {
    AppConfig {
        nowledge_mem: Some(NowledgeMemConfig {
            profiles: Some(HashMap::from([(
                "profile".into(),
                NowledgeMemProfileConfig {
                    endpoint: "https://mem.example/mcp".into(),
                    credential_env: "TEAM_MEM_KEY".into(),
                    tool_set: Some("external-agent".into()),
                },
            )])),
            team_bindings: Some(HashMap::from([(
                "team".into(),
                NowledgeMemTeamBindingConfig {
                    profile: "profile".into(),
                    space_id: "space-a".into(),
                    actor_profiles: None,
                },
            )])),
        }),
        ..Default::default()
    }
}

#[test]
fn configured_mem_pins_configuration_without_pinning_rotating_secrets() {
    let config = config();
    let first = resolve_mem(&config, "team", "worker", |name| {
        assert_eq!(name, "TEAM_MEM_KEY");
        Some("private-first".into())
    })
    .unwrap();
    let rotated = resolve_mem(&config, "team", "worker", |_| {
        Some("private-rotated".into())
    })
    .unwrap();
    assert_eq!(first.fingerprint, rotated.fingerprint);
    assert_eq!(first.binding.server_id(), "nowledge-mem");
    assert_eq!(first.fingerprint.len(), 64);
    let mut changed = config.clone();
    changed
        .nowledge_mem
        .as_mut()
        .unwrap()
        .team_bindings
        .as_mut()
        .unwrap()
        .get_mut("team")
        .unwrap()
        .space_id = "space-b".into();
    assert_ne!(
        first.fingerprint,
        resolve_mem(&changed, "team", "worker", |_| Some("private-first".into()))
            .unwrap()
            .fingerprint
    );
    assert!(
        resolve_mem(&config, "foreign", "worker", |_| Some(
            "private-first".into()
        ))
        .is_err()
    );
}

#[test]
fn configured_mem_rejects_missing_credentials_and_unsafe_endpoint_without_echoing_them() {
    let config = config();
    assert!(resolve_mem(&config, "team", "worker", |_| None).is_err());
    for endpoint in [
        "https://user:private-password@mem.example/mcp",
        "https://mem.example/mcp?key=private-query",
        "http://mem.example/mcp",
        "https://mem.example/other",
        "https://mem.example/mcp#private-fragment",
    ] {
        let mut invalid = config.clone();
        invalid
            .nowledge_mem
            .as_mut()
            .unwrap()
            .profiles
            .as_mut()
            .unwrap()
            .get_mut("profile")
            .unwrap()
            .endpoint = endpoint.into();
        let error = resolve_mem(&invalid, "team", "worker", |_| Some("private-key".into()))
            .err()
            .unwrap()
            .to_string();
        assert!(!error.contains("private") && !error.contains("mem.example"));
    }
    for reference in [
        "HOME",
        "PATH",
        "AGENTHUB_LOOP_CREDENTIAL_FILE",
        "bad=reference",
    ] {
        assert!(!valid_credential_reference(reference));
    }
    for key in ["", "private-key\nheader: injected", "private key"] {
        let error = resolve_mem(&config, "team", "worker", |_| Some(key.into()))
            .err()
            .unwrap()
            .to_string();
        assert!(!error.contains("private"));
    }
}

#[test]
fn private_environment_includes_all_profiles_and_ambient_mem_headers() {
    let names = private_environment(&config());
    for name in [
        "TEAM_MEM_KEY",
        "NMEM_API_KEY",
        "NMEM_API_URL",
        "NOWLEDGE_MEM_HEADERS",
        "MCP_HTTP_HEADERS",
        "nmem_tool_set",
    ] {
        assert!(is_private_environment(name, &names), "{name}");
    }
    for name in [
        "PATH",
        "HOME",
        "OPENAI_API_KEY",
        "AGENTHUB_LOOP_CREDENTIAL_FILE",
    ] {
        assert!(!is_private_environment(name, &names), "{name}");
    }
}
