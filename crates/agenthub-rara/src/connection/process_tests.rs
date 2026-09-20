use std::process::Stdio;

use serde_json::json;
use tokio::io::{AsyncReadExt, BufReader};

use super::*;
use crate::{ControlEnvelope, LaunchCommand, Provenance};

/// Opt-in integration evidence against the pinned source build, without model calls.
#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY built from PINNED_UPSTREAM_REVISION"]
async fn native_process_transport_round_trip() {
    let binary = std::env::var("AGENTHUB_RARA_TEST_BINARY").expect("pinned binary path");
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("workspace with spaces");
    let state = directory.path().join("state");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::create_dir(&state).unwrap();
    // The fixture credential belongs to the child's native configuration, never argv or logs.
    std::fs::write(
        state.join("config.json"),
        serde_json::to_vec(&json!({
            "provider":"deepseek","api_key":"fixture-key","model":"fixture-model",
            "base_url":"http://127.0.0.1:9/v1"
        }))
        .unwrap(),
    )
    .unwrap();
    let config = agenthub_config::RaraConfig {
        binary: Some(binary),
        ..Default::default()
    }
    .resolve_with(|_| None)
    .unwrap();
    let launch = LaunchCommand::new(&config, &workspace).unwrap();
    let mut child = tokio::process::Command::new(launch.program)
        .args(launch.args)
        .args(["--no-extension-discovery", "--no-memory-facilities"])
        .current_dir(&workspace)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("RARA_HOME", &state)
        .env("SHELL", "/bin/sh")
        .env("TERM", "dumb")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let diagnostics = tokio::spawn(async move {
        let mut bytes = [0; 4096];
        let mut count = 0_usize;
        loop {
            let size = stderr.read(&mut bytes).await.unwrap();
            if size == 0 {
                return count;
            }
            count = count.saturating_add(size);
        }
    });
    let mut connection = Connection::open(
        BufReader::new(child.stdout.take().unwrap()),
        child.stdin.take().unwrap(),
        ConnectionOptions::default(),
    )
    .await
    .unwrap();
    let runtime_id = connection.client.handshake().runtime_id.clone();
    assert!(!connection.client.handshake().supports("session.resume"));
    assert!(
        !connection
            .client
            .handshake()
            .capabilities
            .approval_persistence
    );
    let created = connection
        .client
        .request(ClientFrame::Control {
            runtime_id: runtime_id.clone(),
            envelope: ControlEnvelope {
                request_id: "probe-create".into(),
                provenance: Provenance::new(None),
                request: json!({"type":"session","payload":{"type":"create_session"}}),
            },
            expected_turn_id: None,
        })
        .await
        .unwrap();
    let RequestResult::Accepted {
        session_id: Some(session_id),
        ..
    } = created.result
    else {
        panic!("session creation must be accepted");
    };
    assert_ne!(runtime_id, session_id);
    let first = timeout(Duration::from_secs(10), connection.output.recv())
        .await
        .unwrap()
        .unwrap();
    let OutputFrame::Event(first) = first else {
        panic!("initial session event");
    };
    assert_eq!(first.runtime_id, runtime_id);
    assert_eq!(first.session_id, session_id);
    assert_eq!(first.event.sequence, 1);
    let sources = [
        crate::SourceRegistration::Prompt {
            source_id: "loop-context".into(),
            content: "Use the assigned outer activation identity.".into(),
        },
        crate::SourceRegistration::Skill {
            source_id: "loop-skill".into(),
            name: "loop-fixture".into(),
            content: "---\ndescription: Inspect canonical work.\n---\nInspect the current task before acting.".into(),
        },
    ];
    crate::SourceRegistration::validate_batch(&sources, connection.client.handshake()).unwrap();
    for (index, source) in sources.into_iter().enumerate() {
        let frame = crate::ControlRequest::RegisterSource(source)
            .frame(
                &runtime_id,
                &format!("probe-source-{index}"),
                Some(&session_id),
            )
            .unwrap();
        let receipt = connection.client.request(frame).await.unwrap();
        assert!(
            matches!(receipt.result, RequestResult::Accepted { session_id: Some(ref owned), turn_id: None, .. } if owned == &session_id)
        );
    }
    let consumer = tokio::spawn(async move {
        let mut count = 1;
        while let Some(frame) = connection.output.recv().await {
            if let OutputFrame::Event(event) = frame {
                assert_eq!(event.session_id, session_id);
                count += 1;
            } else {
                panic!("unexpected replay gap");
            }
        }
        count
    });
    let receipt = connection
        .client
        .shutdown("probe-shutdown".into())
        .await
        .unwrap();
    assert_eq!(receipt.runtime_id, runtime_id);
    assert_eq!(receipt.request_id, "probe-shutdown");
    assert!(
        timeout(Duration::from_secs(10), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    assert!(consumer.await.unwrap() >= 3);
    let _diagnostic_bytes = timeout(Duration::from_secs(10), diagnostics)
        .await
        .unwrap()
        .unwrap();
}
