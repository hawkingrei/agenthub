use super::*;

#[derive(Default)]
pub(super) struct RecordingMessageArchive {
    documents: Mutex<Vec<MessageDocument>>,
}

#[async_trait]
impl MessageArchiveStore for RecordingMessageArchive {
    async fn ensure_ready(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append_documents(&self, documents: &[MessageDocument]) -> anyhow::Result<()> {
        self.documents.lock().await.extend_from_slice(documents);
        Ok(())
    }

    async fn search(&self, _query: &MessageSearchQuery) -> anyhow::Result<Vec<MessageSearchHit>> {
        Ok(Vec::new())
    }
}

pub(super) struct TailAppendingMessageArchive {
    pub(super) db: SqlitePool,
    pub(super) conversation_id: String,
    pub(super) task_id: String,
    pub(super) run_id: Option<String>,
    pub(super) inserted: Mutex<bool>,
    pub(super) documents: Mutex<Vec<MessageDocument>>,
}

#[test]
fn task_conversation_payload_correlation_id_normalizes_optional_payload_field() {
    assert_eq!(
        task_conversation_payload_correlation_id(&json!({"correlation_id":" corr-1 "})),
        "corr-1"
    );
    assert_eq!(
        task_conversation_payload_correlation_id(&json!({"correlation_id":"   "})),
        ""
    );
    assert_eq!(task_conversation_payload_correlation_id(&json!({})), "");
    assert_eq!(
        task_conversation_payload_correlation_id(&json!({"correlation_id":42})),
        ""
    );
}

#[async_trait]
impl MessageArchiveStore for TailAppendingMessageArchive {
    async fn ensure_ready(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append_documents(&self, documents: &[MessageDocument]) -> anyhow::Result<()> {
        self.documents.lock().await.extend_from_slice(documents);
        let should_insert = {
            let mut inserted = self.inserted.lock().await;
            if *inserted {
                false
            } else {
                *inserted = true;
                true
            }
        };
        if should_insert {
            sqlx::query(
                r#"
                INSERT INTO team_conversation_messages (
                    conversation_id,
                    task_id,
                    from_actor_id,
                    to_actor_id,
                    route,
                    payload_json,
                    created_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
            )
            .bind(&self.conversation_id)
            .bind(&self.task_id)
            .bind("user")
            .bind("coordinator")
            .bind("to_coordinator")
            .bind(json!({"type":"chat_message","text":"live tail message"}).to_string())
            .bind(Utc::now().timestamp())
            .execute(&self.db)
            .await?;
            if let Some(run_id) = self.run_id.as_deref() {
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, NULL, ?2, ?3, ?4)
                    "#,
                )
                .bind(run_id)
                .bind("live_tail_event")
                .bind(Utc::now().timestamp())
                .bind(json!({"text":"live tail run event"}).to_string())
                .execute(&self.db)
                .await?;
                sqlx::query(
                    r#"
                    INSERT INTO team_actor_messages (
                        run_id,
                        from_actor_id,
                        from_peer_id,
                        to_actor_id,
                        to_peer_id,
                        channel,
                        transport,
                        route_json,
                        payload_json,
                        status,
                        created_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10)
                    "#,
                )
                .bind(run_id)
                .bind("coordinator")
                .bind(ACTOR_MAIN_PEER_ID)
                .bind("worker-1")
                .bind(ACTOR_MAIN_PEER_ID)
                .bind("all")
                .bind("local")
                .bind(json!({"type":"chat_message","text":"live tail actor message"}).to_string())
                .bind("pending")
                .bind(Utc::now().timestamp())
                .execute(&self.db)
                .await?;
            }
        }
        Ok(())
    }

    async fn search(&self, _query: &MessageSearchQuery) -> anyhow::Result<Vec<MessageSearchHit>> {
        Ok(Vec::new())
    }
}

pub(super) struct PendingMessageArchive;

#[async_trait]
impl MessageArchiveStore for PendingMessageArchive {
    async fn ensure_ready(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append_documents(&self, _documents: &[MessageDocument]) -> anyhow::Result<()> {
        std::future::pending::<()>().await;
        Ok(())
    }

    async fn search(&self, _query: &MessageSearchQuery) -> anyhow::Result<Vec<MessageSearchHit>> {
        Ok(Vec::new())
    }
}

pub(super) async fn wait_for_archive_documents(
    archive: &RecordingMessageArchive,
    expected_len: usize,
) -> Vec<MessageDocument> {
    timeout(Duration::from_secs(2), async {
        loop {
            let documents = archive.documents.lock().await.clone();
            if documents.len() >= expected_len {
                return documents;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("archive documents should be appended")
}

pub(super) async fn wait_for_archive_run_event_documents(
    archive: &RecordingMessageArchive,
    run_id: &str,
    expected_len: usize,
) -> Vec<MessageDocument> {
    timeout(Duration::from_secs(2), async {
        loop {
            let documents = archive.documents.lock().await.clone();
            let run_event_documents = documents
                .into_iter()
                .filter(|document| {
                    document.source_kind == MessageDocumentKind::TeamRunEvent
                        && document.run_id.as_deref() == Some(run_id)
                })
                .collect::<Vec<_>>();
            if run_event_documents.len() >= expected_len {
                return run_event_documents;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("archive run event documents should be appended")
}

pub(super) fn archived_run_event_types<'a>(
    documents: &[MessageDocument],
    events: &'a [TeamRunEventRecord],
) -> Vec<&'a str> {
    documents
        .iter()
        .filter_map(|document| {
            events
                .iter()
                .find(|event| document.source_id == event.event_id.to_string())
                .map(|event| event.event_type.as_str())
        })
        .collect()
}

pub(super) async fn assert_archive_documents_stay_empty(archive: &RecordingMessageArchive) {
    timeout(Duration::from_secs(2), async {
        loop {
            let documents = archive.documents.lock().await.clone();
            assert!(
                documents.is_empty(),
                "archive should remain empty, got documents: {documents:?}"
            );
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect_err("archive should remain empty for the full assertion window");
}

pub(super) async fn setup_test_db() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect sqlite");

    sqlx::query(
        r#"
        CREATE TABLE team_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            spec_json TEXT NOT NULL,
            owner_user_id TEXT,
            group_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_definitions");

    sqlx::query(
        r#"
        CREATE TABLE team_runs (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            group_id TEXT,
            context_id TEXT NOT NULL,
            status TEXT NOT NULL,
            input_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            started_at INTEGER,
            ended_at INTEGER,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_runs");

    sqlx::query(
        r#"
        CREATE TABLE team_steps (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            step_key TEXT NOT NULL,
            member_id TEXT NOT NULL,
            remote_task_id TEXT,
            status TEXT NOT NULL,
            attempt INTEGER NOT NULL DEFAULT 0,
            depends_on_json TEXT NOT NULL DEFAULT '[]',
            input_json TEXT,
            output_json TEXT,
            error_text TEXT,
            started_at INTEGER,
            ended_at INTEGER,
            UNIQUE(run_id, step_key, attempt),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_steps");

    sqlx::query(
        r#"
        CREATE TABLE team_run_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            step_id TEXT,
            event_type TEXT NOT NULL,
            ts INTEGER NOT NULL,
            payload_json TEXT NOT NULL,
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_run_events");

    sqlx::query(
        r#"
        CREATE TABLE team_tasks (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            group_id TEXT,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            priority TEXT NOT NULL DEFAULT 'medium',
            created_by_actor_id TEXT NOT NULL,
            assigned_member_id TEXT,
            context_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_tasks");

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX idx_team_channel_bootstrap_unique
        ON team_tasks(
            team_id,
            lower(trim(COALESCE(json_extract(context_json, '$.channel_id'), '')))
        )
        WHERE lower(trim(COALESCE(json_extract(context_json, '$.bootstrap_kind'), ''))) = 'team_channel';
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team channel bootstrap unique index");

    sqlx::query(
        r#"
        CREATE TABLE team_conversations (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            task_id TEXT NOT NULL UNIQUE,
            mode TEXT NOT NULL,
            topic TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(task_id) REFERENCES team_tasks(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_conversations");

    sqlx::query(
        r#"
        CREATE TABLE team_conversation_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            to_actor_id TEXT,
            route TEXT NOT NULL,
            correlation_id TEXT NOT NULL DEFAULT '',
            group_id TEXT,
            payload_json TEXT NOT NULL,
            idempotency_key TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES team_conversations(id),
            FOREIGN KEY(task_id) REFERENCES team_tasks(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_conversation_messages");

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX idx_team_conversation_messages_idempotency
        ON team_conversation_messages(conversation_id, from_actor_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL;
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_conversation_messages idempotency index");

    sqlx::query(
        r#"
        CREATE TABLE team_actor_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            from_peer_id TEXT NOT NULL DEFAULT 'main',
            to_actor_id TEXT NOT NULL,
            to_peer_id TEXT NOT NULL DEFAULT 'main',
            channel TEXT NOT NULL,
            transport TEXT NOT NULL,
            route_json TEXT,
            payload_json TEXT NOT NULL,
            message_kind TEXT NOT NULL DEFAULT 'coordination_request',
            group_id TEXT,
            idempotency_key TEXT,
            status TEXT NOT NULL,
            handling_disposition TEXT NOT NULL DEFAULT 'untriaged',
            handled_by_actor_id TEXT,
            handled_at INTEGER,
            created_at INTEGER NOT NULL,
            delivered_at INTEGER,
            relay_attempt INTEGER NOT NULL DEFAULT 0,
            relay_next_retry_at INTEGER,
            relay_last_error TEXT,
            dead_letter_at INTEGER,
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_actor_messages");

    sqlx::query(
        r#"
        CREATE TABLE team_actor_thread_claims (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            topic_key TEXT NOT NULL,
            task_id TEXT,
            root_message_id INTEGER,
            owner_actor_id TEXT NOT NULL,
            claim_status TEXT NOT NULL,
            claimed_message_id INTEGER,
            claimed_at INTEGER NOT NULL,
            lease_expires_at INTEGER,
            updated_at INTEGER NOT NULL,
            UNIQUE(run_id, topic_key)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_actor_thread_claims");

    sqlx::query(
        r#"
        CREATE TABLE team_actor_message_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            message_id INTEGER NOT NULL,
            task_id TEXT NOT NULL,
            relation TEXT NOT NULL,
            created_by_actor_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE(run_id, message_id, task_id, relation)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_actor_message_links");

    sqlx::query(
        r#"
        CREATE TABLE team_channel_message_replicas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            authority_message_id INTEGER NOT NULL,
            correlation_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            group_id TEXT,
            from_actor_id TEXT NOT NULL,
            source_node_id TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            stored_at INTEGER NOT NULL,
            UNIQUE(authority_message_id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_channel_message_replicas");

    sqlx::query(
        r#"
        CREATE TABLE team_member_continuity_state (
            team_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            source_run_id TEXT NOT NULL,
            source_session_id TEXT,
            summary_text TEXT NOT NULL,
            history_window_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (team_id, member_id),
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(source_run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_member_continuity_state");

    sqlx::query(
        r#"
        CREATE TABLE team_context_artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            team_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            session_id TEXT,
            artifact_seq INTEGER NOT NULL,
            artifact_kind TEXT NOT NULL,
            artifact_path TEXT NOT NULL,
            artifact_size_bytes INTEGER NOT NULL,
            content_checksum TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_context_artifacts");

    sqlx::query(
        r#"
        CREATE TABLE team_context_flush_checkpoint (
            team_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            last_event_id INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (run_id, member_id, session_id),
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_context_flush_checkpoint");

    sqlx::query(
        r#"
        CREATE TABLE agents (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            workdir TEXT NOT NULL,
            command TEXT NOT NULL,
            args TEXT NOT NULL,
            worktree_mode TEXT NOT NULL,
            worktree_repo TEXT,
            worktree_ref TEXT,
            code_mode INTEGER NOT NULL DEFAULT 0,
            agent_loop_enabled INTEGER NOT NULL DEFAULT 0,
            agent_loop_idle_seconds INTEGER,
            agent_loop_prompt TEXT,
            source TEXT NOT NULL DEFAULT 'manual',
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create agents");

    sqlx::query(
        r#"
        CREATE TABLE agent_sessions (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at INTEGER NOT NULL,
            ended_at INTEGER,
            FOREIGN KEY(agent_id) REFERENCES agents(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create agent_sessions");

    sqlx::query(
        r#"
        CREATE TABLE agent_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            seq TEXT NOT NULL,
            ts INTEGER NOT NULL,
            stream TEXT NOT NULL,
            message BLOB NOT NULL,
            FOREIGN KEY(agent_id) REFERENCES agents(id),
            FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("create agent_events");

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX idx_team_actor_messages_idempotency
        ON team_actor_messages(run_id, from_actor_id, from_peer_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL
        "#,
    )
    .execute(&pool)
    .await
    .expect("create team_actor_messages idempotency index");

    pool
}

pub(super) fn task_attempt_number(task: &crate::team::TeamTaskRecord) -> Option<i64> {
    task.context
        .get("execution")
        .and_then(|value| value.get("attempt_number"))
        .and_then(Value::as_i64)
}

pub(super) async fn insert_team_conversation_message(
    db: &SqlitePool,
    conversation_id: &str,
    task_id: &str,
    from_actor_id: &str,
    payload_json: Value,
) -> i64 {
    let created_at = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO team_conversation_messages (
            conversation_id, task_id, from_actor_id, to_actor_id, route, group_id, payload_json, idempotency_key, created_at
        )
        SELECT ?1, ?2, ?3, NULL, 'broadcast', group_id, ?4, NULL, ?5
        FROM team_tasks
        WHERE id = ?2
        "#,
    )
    .bind(conversation_id)
    .bind(task_id)
    .bind(from_actor_id)
    .bind(payload_json.to_string())
    .bind(created_at)
    .execute(db)
    .await
    .expect("insert team conversation message")
    .last_insert_rowid()
}

#[derive(Debug, Clone)]
pub(super) struct RelayHttpCapture {
    pub(super) method: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: serde_json::Value,
}

#[derive(Clone)]
pub(super) struct RelayHttpState {
    captures: Arc<Mutex<Vec<RelayHttpCapture>>>,
    status: StatusCode,
}

pub(super) async fn relay_http_handler(
    State(state): State<RelayHttpState>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, &'static str) {
    let mut captured_headers = HashMap::new();
    for (name, value) in &headers {
        if let Ok(text) = value.to_str() {
            captured_headers.insert(name.as_str().to_string(), text.to_string());
        }
    }
    let captured_body = serde_json::from_slice::<serde_json::Value>(&body)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&body).to_string()));
    state.captures.lock().await.push(RelayHttpCapture {
        method: method.as_str().to_string(),
        headers: captured_headers,
        body: captured_body,
    });
    (state.status, "ok")
}

pub(super) async fn spawn_relay_http_server(
    status: StatusCode,
) -> (
    String,
    Arc<Mutex<Vec<RelayHttpCapture>>>,
    tokio::task::JoinHandle<()>,
) {
    let captures = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new()
        .route("/relay", any(relay_http_handler))
        .route("/health", any(|| async { StatusCode::OK }))
        .with_state(RelayHttpState {
            captures: captures.clone(),
            status,
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind relay test server");
    let addr = listener.local_addr().expect("relay local addr");
    let handle = tokio::spawn(async move {
        let _ = serve(listener, app).await;
    });

    let endpoint = format!("http://{addr}/relay");
    let health_endpoint = format!("http://{addr}/health");
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(std::time::Duration::from_millis(50))
        .timeout(std::time::Duration::from_millis(100))
        .build()
        .expect("build probe client");
    let mut ready = false;
    for _ in 0..50 {
        if let Ok(resp) = client.get(&health_endpoint).send().await
            && resp.status().is_success()
        {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    if !ready {
        panic!("relay test server failed to become ready");
    }

    (endpoint, captures, handle)
}

#[test]
fn team_run_status_from_str_handles_submitted_and_unknown_values() {
    assert_eq!(
        team_run_status_from_str("submitted"),
        TeamRunStatus::Submitted
    );
    assert_eq!(
        team_run_status_from_str("unexpected"),
        TeamRunStatus::Submitted
    );
}
