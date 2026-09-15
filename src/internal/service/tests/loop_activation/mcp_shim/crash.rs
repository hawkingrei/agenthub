use std::{path::PathBuf, process::Stdio, sync::atomic::AtomicUsize};

use crate::{api::team_tests, daemon_instance::DaemonInstanceGuard};

use super::*;

mod fixture;
use fixture::{
    Ready, Shim, binding, credential, pause_success_commit, serve, statuses, wait_commit_pause,
    wait_ready, wait_status,
};

const CHILD: &str =
    "internal::service::tests::loop_activation::mcp_shim::crash::mcp_crash_daemon_child";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    BeforeCall,
    SentWithoutResponse,
    ResponseBeforeCommit,
    DurableSuccess,
}

impl Boundary {
    fn is_ambiguous(self) -> bool {
        matches!(self, Self::SentWithoutResponse | Self::ResponseBeforeCommit)
    }
}

#[tokio::test]
#[ignore = "Started and killed by the parent with an isolated file-backed control database"]
async fn mcp_crash_daemon_child() {
    let directory = PathBuf::from(std::env::var("AGENTHUB_TEST_MCP_CRASH_DIR").unwrap());
    let endpoint = std::env::var("AGENTHUB_TEST_MCP_CRASH_ENDPOINT").unwrap();
    let db_path = directory.join("control.sqlite");
    let state = team_tests::build_test_state_with_db_path(&db_path).await;
    let (state, service, _, run, reservation) = super::super::fixture_with_state(state, true).await;
    agenthub_db::mcp_operations::migrate_mcp_operations(&state.db)
        .await
        .unwrap();
    let mut daemon = DaemonInstanceGuard::acquire(&db_path, "main").unwrap();
    daemon.claim_generation(&state.db).await.unwrap();
    if std::env::var_os("AGENTHUB_TEST_MCP_PAUSE_SUCCESS").is_some() {
        pause_success_commit(&state.db, &directory).await;
    }
    state
        .agents
        .initialize_mcp_proxy(
            daemon.mcp_operation_store(&state.db).unwrap(),
            vec![(reservation.clone(), binding(&endpoint))],
        )
        .unwrap();
    let (target, _server) = serve(service).await;
    let ready = Ready {
        reservation,
        run_id: run.id,
        target,
    };
    std::fs::write(
        directory.join("ready.tmp"),
        serde_json::to_vec(&ready).unwrap(),
    )
    .unwrap();
    std::fs::rename(directory.join("ready.tmp"), directory.join("ready.json")).unwrap();
    // The parent kills this process without running journal or runtime shutdown handlers.
    std::future::pending::<()>().await;
    drop(daemon);
}

#[tokio::test]
async fn real_mcp_proxy_survives_daemon_crashes_without_replaying_an_unknown_write() {
    for boundary in [
        Boundary::BeforeCall,
        Boundary::SentWithoutResponse,
        Boundary::ResponseBeforeCommit,
        Boundary::DurableSuccess,
    ] {
        tokio::time::timeout(Duration::from_secs(30), crash_case(boundary))
            .await
            .unwrap_or_else(|_| panic!("crash case timed out: {boundary:?}"));
    }
}

async fn crash_case(boundary: Boundary) {
    let directory = std::env::temp_dir().join(format!("agenthub-mcp-crash-{}", Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let db_path = directory.join("control.sqlite");
    let calls = Arc::new(AtomicUsize::new(0));
    let received = Arc::new(Notify::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
    let router = axum::Router::new().route("/mcp", post({
        let db_path = db_path.clone();
        let calls = calls.clone();
        let received = received.clone();
        move |Json(message): Json<Value>| {
            let db_path = db_path.clone();
            let calls = calls.clone();
            let received = received.clone();
            async move {
                let result = match message["method"].as_str().unwrap() {
                    "tools/list" => json!({"resultType":"complete","tools":[{"name":"write","inputSchema":{"type":"object","properties":{"body":{"type":"string"}}}}]}),
                    "tools/call" => {
                        let pool = sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&db_path)).await.unwrap();
                        assert_eq!(statuses(&pool).await, ["sent"]);
                        pool.close().await;
                        calls.fetch_add(1, Ordering::SeqCst);
                        received.notify_one();
                        if boundary == Boundary::SentWithoutResponse {
                            std::future::pending::<()>().await;
                        }
                        json!({"resultType":"complete","content":[{"type":"text","text":"private-crash-result"}]})
                    }
                    _ => panic!("unexpected crash fixture method"),
                };
                Json(json!({"jsonrpc":"2.0","id":message["id"],"result":result}))
            }
        }
    }));
    let http = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let log = std::fs::File::create(directory.join("child.log")).unwrap();
    let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", CHILD, "--ignored", "--nocapture"])
        .env("AGENTHUB_TEST_MCP_CRASH_DIR", &directory)
        .env("AGENTHUB_TEST_MCP_CRASH_ENDPOINT", &endpoint)
        .env_remove("AGENTHUB_TEST_MCP_PAUSE_SUCCESS")
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .kill_on_drop(true);
    if boundary == Boundary::ResponseBeforeCommit {
        command.env("AGENTHUB_TEST_MCP_PAUSE_SUCCESS", "1");
    }
    let mut daemon_child = command.spawn().unwrap();
    let ready = wait_ready(&directory, &mut daemon_child).await;
    let mut shim = Shim::start(&ready).await;
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(&db_path),
    )
    .await
    .unwrap();
    let arguments = json!({"name":"write","arguments":{"body":"private-crash-body"}});
    if boundary != Boundary::BeforeCall {
        shim.send(2, "tools/call", arguments.clone()).await;
        tokio::time::timeout(Duration::from_secs(5), received.notified())
            .await
            .unwrap();
        wait_status(
            &pool,
            if boundary == Boundary::DurableSuccess {
                "succeeded"
            } else {
                "sent"
            },
        )
        .await;
    } else {
        assert!(statuses(&pool).await.is_empty());
    }
    if boundary == Boundary::ResponseBeforeCommit {
        wait_commit_pause(&directory, &mut daemon_child).await;
        assert_eq!(statuses(&pool).await, ["sent"]);
        shim.assert_no_output().await;
    }
    // Leave the provider response unread, including when its success is already durable.
    daemon_child.kill().await.unwrap();
    assert!(!daemon_child.wait().await.unwrap().success());
    if shim.child.try_wait().unwrap().is_none() {
        shim.child.kill().await.unwrap();
    }
    drop(shim);
    pool.close().await;

    // The OS lock and a new generation, rather than elapsed time, establish new daemon ownership.
    let mut daemon = DaemonInstanceGuard::acquire(&db_path, "main").unwrap();
    let state = team_tests::reopen_test_state_with_db_path(&db_path).await;
    daemon.claim_generation(&state.db).await.unwrap();
    let journal = daemon.mcp_operation_store(&state.db).unwrap();
    assert_eq!(
        journal
            .recover_interrupted(100, chrono::Utc::now().timestamp())
            .await
            .unwrap(),
        u64::from(boundary.is_ambiguous())
    );
    assert_eq!(
        journal
            .recover_interrupted(100, chrono::Utc::now().timestamp())
            .await
            .unwrap(),
        0
    );
    let expected = match boundary {
        Boundary::BeforeCall => vec![],
        Boundary::SentWithoutResponse | Boundary::ResponseBeforeCommit => {
            vec!["outcome_unknown".to_owned()]
        }
        Boundary::DurableSuccess => vec!["succeeded".to_owned()],
    };
    assert_eq!(statuses(&state.db).await, expected);
    if boundary.is_ambiguous() {
        let completion: String = sqlx::query_scalar("SELECT completion_json FROM mcp_operations")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&completion).unwrap(),
            json!({"kind":"outcome_unknown","reason":"daemon_restart"})
        );
    }
    let run = state.teams.get_run(&ready.run_id).await.unwrap();
    let loops = LoopStore::new(state.db.clone());
    let now = chrono::Utc::now().timestamp();
    loops
        .cancel(
            &run.team_id,
            ready.reservation.activation_id.as_deref().unwrap(),
            now,
        )
        .await
        .unwrap();
    loops
        .cleanup_verified(&ready.reservation, LoopCleanupDisposition::Exited, now)
        .await
        .unwrap();
    sqlx::query("UPDATE agent_sessions SET status = 'completed' WHERE id = ?")
        .bind(&ready.reservation.session_id)
        .execute(&state.db)
        .await
        .unwrap();
    let next = super::super::reserve_fixture(&state, &run, "after-crash", true).await;
    state
        .agents
        .initialize_mcp_proxy(journal.clone(), vec![(next.clone(), binding(&endpoint))])
        .unwrap();
    let service = TeamInternalControlService::new(
        control_deps(&state),
        build_authz(),
        InternalGrpcSecurityMode::Disabled,
        directory.clone(),
        "bootstrap".into(),
    );
    let stale = service
        .open_mcp_proxy(authenticated_request(
            OpenMcpProxyRequest {
                server_id: "fixture".into(),
            },
            &credential(&ready),
        ))
        .await;
    assert!(stale.is_err(), "old executor credentials survived restart");
    let (target, grpc) = serve(service).await;
    let mut restarted = Shim::start(&Ready {
        reservation: next,
        run_id: run.id,
        target,
    })
    .await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        usize::from(boundary != Boundary::BeforeCall)
    );
    match boundary {
        Boundary::BeforeCall => {
            restarted.send(88, "tools/call", arguments).await;
            assert!(restarted.receive().await.get("result").is_some());
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(statuses(&state.db).await, ["succeeded"]);
        }
        Boundary::SentWithoutResponse | Boundary::ResponseBeforeCommit => {
            restarted.send(88, "tools/call", arguments).await;
            let denied = restarted.receive().await;
            assert_eq!(denied["id"], 88);
            assert!(denied.get("error").is_some());
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(statuses(&state.db).await, ["outcome_unknown"]);
        }
        Boundary::DurableSuccess => {
            assert_eq!(statuses(&state.db).await, ["succeeded"]);
        }
    }
    let records: Vec<String> = sqlx::query_scalar(
        "SELECT intent_json || COALESCE(completion_json, '') FROM mcp_operations",
    )
    .fetch_all(&state.db)
    .await
    .unwrap();
    for private in ["private-crash-body", "private-crash-result", &endpoint] {
        assert!(!records.join("").contains(private));
    }
    restarted.finish().await;
    state
        .agents
        .daemon_tasks()
        .shutdown_runtime(Duration::from_secs(3))
        .await
        .unwrap();
    grpc.abort();
    http.abort();
    state.db.close().await;
    drop(daemon);
    std::fs::remove_dir_all(directory).unwrap();
}
