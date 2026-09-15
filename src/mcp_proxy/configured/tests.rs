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

#[tokio::test]
async fn configured_mem_pins_configuration_without_pinning_rotating_secrets() {
    use axum::{Json, http::HeaderMap, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let router = axum::Router::new().route(
            "/members/me",
            get(|headers: HeaderMap| async move {
                let space = if headers["authorization"] == "Bearer space-b-key" {
                    "space-b"
                } else {
                    "space-a"
                };
                let workspace = if headers["authorization"] == "Bearer foreign-workspace-key" {
                    "9405e041-1948-46d4-bc91-404e64ab6006"
                } else {
                    "cd270331-80bc-4f90-8cc0-3fefbc7f74ab"
                };
                Json(json!({"workspace_id":workspace,
                "key_scope":{"scope_mode":"narrowed","grants":[space],"write_space":space},
                "key_write_target":{"write_space":space,"write_space_live":true}}))
            }),
        );
        axum::serve(listener, router).await.unwrap();
    });
    let mut config = config();
    config
        .nowledge_mem
        .as_mut()
        .unwrap()
        .profiles
        .as_mut()
        .unwrap()
        .get_mut("profile")
        .unwrap()
        .endpoint = endpoint;
    let first = resolve_mem(&config, "team", "worker", |name| {
        assert_eq!(name, "TEAM_MEM_KEY");
        Some("private-first".into())
    })
    .await
    .unwrap();
    let rotated = resolve_mem(&config, "team", "worker", |_| {
        Some("private-rotated".into())
    })
    .await
    .unwrap();
    assert_eq!(first.fingerprint, rotated.fingerprint);
    assert_eq!(first.binding.server_id(), "nowledge-mem");
    assert_eq!(first.fingerprint.len(), 64);
    let mut overridden = config.clone();
    let mem = overridden.nowledge_mem.as_mut().unwrap();
    let mut other_profile = mem.profiles.as_ref().unwrap()["profile"].clone();
    other_profile.credential_env = "OTHER_MEM_KEY".into();
    mem.profiles
        .as_mut()
        .unwrap()
        .insert("other".into(), other_profile);
    mem.team_bindings
        .as_mut()
        .unwrap()
        .get_mut("team")
        .unwrap()
        .actor_profiles = Some(HashMap::from([("worker".into(), "other".into())]));
    assert!(
        resolve_mem(&overridden, "team", "worker", |reference| {
            Some(
                if reference == "OTHER_MEM_KEY" {
                    "space-b-key"
                } else {
                    "private-first"
                }
                .into(),
            )
        })
        .await
        .is_err(),
        "an actor profile cannot change the Team's namespace"
    );
    let error = resolve_mem(&overridden, "team", "worker", |reference| {
        Some(
            if reference == "OTHER_MEM_KEY" {
                "foreign-workspace-key"
            } else {
                "private-first"
            }
            .into(),
        )
    })
    .await
    .err()
    .unwrap();
    assert_eq!(
        error.to_string(),
        "Mem actor profile does not match the Team workspace"
    );
    assert!(
        resolve_mem(&overridden, "team", "worker", |_| Some(
            "private-first".into()
        ))
        .await
        .is_ok()
    );
    assert!(
        validate_mem_configuration(&overridden, "team", "worker", |reference| {
            (reference == "OTHER_MEM_KEY").then(|| "private-first".into())
        })
        .is_err(),
        "the default Team authority also needs a credential reference"
    );
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
        resolve_mem(&changed, "team", "worker", |_| Some("space-b-key".into()))
            .await
            .unwrap()
            .fingerprint
    );
    assert!(
        resolve_mem(&config, "foreign", "worker", |_| Some(
            "private-first".into()
        ))
        .await
        .is_err()
    );
    server.abort();
}

#[test]
fn configured_mem_rejects_missing_credentials_and_unsafe_endpoint_without_echoing_them() {
    let config = config();
    assert!(resolve_connection(&config, "team", "worker", |_| None).is_err());
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
        let error = resolve_connection(&invalid, "team", "worker", |_| Some("private-key".into()))
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
        let error = resolve_connection(&config, "team", "worker", |_| Some(key.into()))
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
