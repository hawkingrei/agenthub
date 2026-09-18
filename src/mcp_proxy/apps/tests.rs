use agenthub_agent_domain::app_tools::{APP_ARGUMENT_MAX_BYTES, AppManifest, AppTool};

use super::*;

fn pin() -> AppActivationPin {
    AppActivationPin {
        activation_id: "activation".into(),
        app_id: "app".into(),
        team_id: "team".into(),
        actor_id: "actor".into(),
        pinned_generation: 1,
        version: 1,
        scopes: ["write".into()].into(),
        grant_revision: 1,
        binding_revision: 1,
        grant_epoch: 1,
        binding_epoch: 1,
    }
}

#[test]
fn app_headers_are_sensitive_and_credentials_are_optional_and_bounded() {
    let mut connection = AppConnection {
        endpoint: "http://127.0.0.1/mcp".into(),
        credential_env: None,
        authority: "fixture".into(),
        namespace: "namespace".into(),
    };
    let headers = connection_headers(&connection, &pin(), "workspace-digest", |_| {
        panic!("no credential configured")
    })
    .unwrap();
    assert!(!headers.contains_key(AUTHORIZATION));
    for (name, value) in [
        ("app-id", "app"),
        ("app-version", "1"),
        ("team-id", "team"),
        ("actor-id", "actor"),
        ("activation-id", "activation"),
        ("workspace", "workspace-digest"),
    ] {
        assert_eq!(headers[format!("x-agenthub-{name}")], value);
    }
    assert!(headers.values().all(HeaderValue::is_sensitive));
    connection.credential_env = Some("APP_PRIVATE_TOKEN".into());
    let headers = connection_headers(&connection, &pin(), "workspace-digest", |reference| {
        assert_eq!(reference, "APP_PRIVATE_TOKEN");
        Some("private-key".into())
    })
    .unwrap();
    assert_eq!(headers[AUTHORIZATION], "Bearer private-key");
    assert!(headers[AUTHORIZATION].is_sensitive());
    for value in [
        None,
        Some(String::new()),
        Some("private key".into()),
        Some("private\nkey".into()),
        Some("x".repeat(8193)),
    ] {
        let error = connection_headers(&connection, &pin(), "workspace-digest", |_| value)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private") && !error.contains("APP_PRIVATE_TOKEN"));
    }
}

#[test]
fn app_result_bounds_cover_native_content_and_error_results() {
    let manifest = AppManifest {
        schema_version: 1,
        scopes: ["write".into()].into(),
        tools: vec![AppTool {
            name: "write".into(),
            input_schema: json!({"type":"object"}),
            output_schema: Some(
                json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"]}),
            ),
            required_scopes: ["write".into()].into(),
            replay: AppReplayPolicy::NonIdempotent,
        }],
    }
    .compile()
    .unwrap();
    assert!(
        validate_result(
            &manifest,
            "write",
            &json!({"content":[],"structuredContent":{"ok":true}})
        )
        .is_ok()
    );
    for value in [
        json!({"content":[]}),
        json!({"content":[],"structuredContent":{"ok":"invalid"}}),
        json!([]),
    ] {
        assert!(validate_result(&manifest, "write", &value).is_err());
    }
    assert!(validate_result(&manifest, "write", &json!({"content":[],"isError":true})).is_ok());
    for error in [false, true] {
        let oversized = json!({"content":[{"type":"text","text":"x".repeat(APP_ARGUMENT_MAX_BYTES)}],"structuredContent":{"ok":true},"isError":error});
        assert!(validate_result(&manifest, "write", &oversized).is_err());
    }
}
