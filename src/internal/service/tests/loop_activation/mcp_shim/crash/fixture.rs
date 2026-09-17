use std::{path::Path, process::Stdio};

use super::*;

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Ready {
    pub reservation: LoopReservation,
    pub run_id: String,
    pub target: String,
}

pub(super) fn binding(endpoint: &str) -> Arc<McpProxyBinding> {
    Arc::new(McpProxyBinding::new(
        McpBinding::new(
            "fixture".into(),
            &json!({"service":"crash-fixture","space":"space-a"}),
            &json!({"revision":1}),
            McpHttpTransport::new(endpoint, HeaderMap::new(), Duration::from_secs(30)).unwrap(),
            BTreeMap::new(),
        )
        .unwrap(),
        agenthub_mcp::access::McpAccessPolicy::tools_only(),
        Arc::new(|_, _, arguments| Ok(arguments)),
    ))
}

pub(super) async fn serve(
    service: TeamInternalControlService,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("http://{}", listener.local_addr().unwrap());
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    });
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(TeamInternalControlServer::new(service))
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    (target, server)
}

pub(super) async fn pause_success_commit(pool: &sqlx::SqlitePool, directory: &Path) {
    // This fixture's pool has one connection. TEMP state and the hook disappear on process exit.
    let mut connection = pool.acquire().await.unwrap();
    sqlx::raw_sql(
        "CREATE TEMP TABLE crash_success_marker (id INTEGER); \
        CREATE TEMP TRIGGER pause_success_commit AFTER UPDATE ON main.mcp_operation_attempts \
        WHEN NEW.status = 'succeeded' BEGIN INSERT INTO crash_success_marker VALUES (1); END;",
    )
    .execute(&mut *connection)
    .await
    .unwrap();
    let marker = directory.join("success-before-commit");
    connection
        .lock_handle()
        .await
        .unwrap()
        .set_update_hook(move |event| {
            if event.database == "temp"
                && event.table == "crash_success_marker"
                && event.operation == sqlx::sqlite::SqliteOperation::Insert
            {
                std::fs::write(&marker, b"success transaction is uncommitted").unwrap();
                loop {
                    std::thread::park();
                }
            }
        });
}

pub(super) async fn wait_commit_pause(directory: &Path, child: &mut tokio::process::Child) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !directory.join("success-before-commit").exists() {
            assert!(
                child.try_wait().unwrap().is_none(),
                "daemon exited before success commit pause"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("success did not reach the uncommitted update hook");
}

pub(super) struct Shim {
    pub child: tokio::process::Child,
    input: tokio::process::ChildStdin,
    output: BufReader<tokio::process::ChildStdout>,
    _credential: LoopCredentialFile,
}

pub(super) fn credential(ready: &Ready) -> String {
    signed_token(
        &build_authz(),
        &ready.reservation,
        &ready.run_id,
        vec![InternalAction::McpProxy.as_str().into()],
    )
}

impl Shim {
    pub async fn start(ready: &Ready) -> Self {
        let file = LoopCredentialFile::create().unwrap();
        file.replace(&LoopCredentialEnvelope {
            actor_id: ready.reservation.actor_id.clone(),
            run_id: ready.run_id.clone(),
            activation_id: ready.reservation.activation_id.clone().unwrap(),
            generation: ready.reservation.generation,
            target: ready.target.clone(),
            access_token: credential(ready),
            expires_at: chrono::Utc::now().timestamp() + 600,
            ca_cert_path: None,
        })
        .unwrap();
        let mut child = tokio::process::Command::new(
            crate::agenthub_binary::resolve_agenthub_binary_path().unwrap(),
        )
        .args(["mcp-proxy", "--server-id", "fixture"])
        .env(LOOP_CREDENTIAL_FILE_ENV, &file.path)
        .env_remove("AGENTHUB_INTERNAL_GRPC_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut shim = Self {
            child,
            input,
            output,
            _credential: file,
        };
        shim.send(1, "tools/list", json!({})).await;
        assert_eq!(shim.receive().await["result"]["tools"][0]["name"], "write");
        shim
    }

    pub async fn send(&mut self, id: i64, method: &str, mut params: Value) {
        params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}});
        write_message(
            &mut self.input,
            &json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        )
        .await
        .unwrap();
    }

    pub async fn receive(&mut self) -> Value {
        tokio::time::timeout(Duration::from_secs(5), read_message(&mut self.output))
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    }

    pub async fn assert_no_output(&mut self) {
        assert!(
            tokio::time::timeout(Duration::from_millis(100), self.output.read_u8())
                .await
                .is_err(),
            "provider received output before the success transaction committed"
        );
    }

    pub async fn finish(mut self) {
        drop(self.input);
        let status = tokio::time::timeout(Duration::from_secs(5), self.child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success());
    }
}

pub(super) async fn wait_ready(directory: &Path, child: &mut tokio::process::Child) -> Ready {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(bytes) = std::fs::read(directory.join("ready.json")) {
                return serde_json::from_slice(&bytes).unwrap();
            }
            if let Some(status) = child.try_wait().unwrap() {
                panic!(
                    "crash fixture exited {status}: {}",
                    std::fs::read_to_string(directory.join("child.log")).unwrap()
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("crash fixture did not become ready")
}

pub(super) async fn statuses(pool: &sqlx::SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT status FROM mcp_operation_attempts ORDER BY operation_id, number")
        .fetch_all(pool)
        .await
        .unwrap()
}

pub(super) async fn wait_status(pool: &sqlx::SqlitePool, expected: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if statuses(pool).await == [expected] {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("journal did not reach the crash boundary")
}
