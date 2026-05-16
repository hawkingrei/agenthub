use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;

use super::codec::is_acp_message;
use super::{AgentHandle, AgentManager};
use crate::agent::event_message_codec::persist_agent_event;
use crate::agent::{AgentOutput, OutputStream};
use crate::push::PushService;
use agenthub_db::{AgentEventDbRouter, AgentEventIdleGc};

impl AgentManager {
    fn handle_matches_session(handle: Option<&AgentHandle>, session_id: &str) -> bool {
        handle
            .map(|handle| handle.session_id.as_str() == session_id)
            .unwrap_or(false)
    }

    pub(super) async fn emit_run_status(
        &self,
        output_tx: broadcast::Sender<AgentOutput>,
        agent_id: String,
        session_id: String,
        status: &str,
    ) {
        let message = serde_json::json!({
            "type": "run_status",
            "status": status,
            "session_id": session_id,
        })
        .to_string();
        let seq = Uuid::now_v7().to_string();
        let ts = Utc::now().timestamp();
        let event_id = match persist_agent_event(
            &self.event_dbs,
            self.idle_gc.as_ref(),
            &agent_id,
            &session_id,
            &seq,
            ts,
            &OutputStream::Acp,
            &message,
        )
        .await
        {
            Ok(event_id) => event_id,
            Err(err) => {
                tracing::error!(
                    agent_id = %agent_id,
                    session_id = %session_id,
                    status = %status,
                    error = %err,
                    "emit_run_status failed to persist event"
                );
                return;
            }
        };
        let output = AgentOutput {
            event_id,
            agent_id: agent_id.clone(),
            session_id: session_id.clone(),
            seq: seq.clone(),
            ts,
            stream: OutputStream::Acp,
            message: message.clone(),
        };
        let _ = output_tx.send(output);
    }

    pub(super) async fn spawn_output_reader<R>(
        &self,
        agent_id: String,
        session_id: String,
        stream: OutputStream,
        reader: R,
        output_tx: broadcast::Sender<AgentOutput>,
        detect_acp_messages: bool,
    ) where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let event_dbs = self.event_dbs.clone();
        let idle_gc = self.idle_gc.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let is_acp = detect_acp_messages && is_acp_message(&line);
                let stream = if is_acp {
                    OutputStream::Acp
                } else {
                    stream.clone()
                };
                let seq = Uuid::now_v7().to_string();
                let ts = Utc::now().timestamp();
                let event_id = match persist_agent_event(
                    &event_dbs,
                    idle_gc.as_ref(),
                    &agent_id,
                    &session_id,
                    &seq,
                    ts,
                    &stream,
                    &line,
                )
                .await
                {
                    Ok(event_id) => event_id,
                    Err(err) => {
                        tracing::error!(
                            agent_id = %agent_id,
                            session_id = %session_id,
                            stream = ?stream,
                            error = %err,
                            "spawn_output_reader failed to persist event"
                        );
                        continue;
                    }
                };
                let output = AgentOutput {
                    event_id,
                    agent_id: agent_id.clone(),
                    session_id: session_id.clone(),
                    seq: seq.clone(),
                    ts,
                    stream: stream.clone(),
                    message: line.clone(),
                };
                let _ = output_tx.send(output);
            }
        });
    }

    pub(super) async fn spawn_exit_watcher(&self, agent_id: String, session_id: String) {
        let db = self.db.clone();
        let event_dbs = self.event_dbs.clone();
        let idle_gc = self.idle_gc.clone();
        let inner = self.inner.clone();
        let push = self.push.clone();
        let agent_id_clone = agent_id.clone();
        tokio::spawn(async move {
            let child = {
                let guard = inner.read().await;
                guard.get(&agent_id).map(|h| h.child.clone())
            };

            if let Some(child_mutex) = child {
                let success = loop {
                    let poll_result = {
                        let mut child_guard = child_mutex.lock().await;
                        let child = match child_guard.as_mut() {
                            Some(child) => child,
                            None => return,
                        };
                        child.try_wait()
                    };
                    match poll_result {
                        Ok(Some(status)) => break status.success(),
                        Ok(None) => {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        Err(err) => {
                            tracing::warn!(
                                agent_id = %agent_id_clone,
                                session_id = %session_id,
                                error = %err,
                                "spawn_exit_watcher failed to poll child status"
                            );
                            break false;
                        }
                    }
                };
                Self::finalize_process_exit(
                    &db,
                    &event_dbs,
                    idle_gc,
                    &inner,
                    &push,
                    &agent_id_clone,
                    &session_id,
                    success,
                )
                .await;
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn finalize_process_exit(
        db: &SqlitePool,
        event_dbs: &AgentEventDbRouter,
        idle_gc: Option<AgentEventIdleGc>,
        inner: &Arc<RwLock<HashMap<String, AgentHandle>>>,
        push: &Arc<PushService>,
        agent_id: &str,
        session_id: &str,
        success: bool,
    ) {
        let row = sqlx::query(
            r#"
            SELECT ended_at FROM agent_sessions WHERE id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_optional(db)
        .await;
        let row = match row {
            Ok(row) => row,
            Err(err) => {
                tracing::warn!(
                    agent_id = %agent_id,
                    session_id = %session_id,
                    error = %err,
                    "finalize_process_exit failed to read existing session state"
                );
                None
            }
        };
        if row.is_none() {
            tracing::debug!(
                agent_id = %agent_id,
                session_id = %session_id,
                "finalize_process_exit skipped because agent session row no longer exists"
            );
            let removed = {
                let mut guard = inner.write().await;
                if Self::handle_matches_session(guard.get(agent_id), session_id) {
                    guard.remove(agent_id);
                    true
                } else {
                    false
                }
            };
            if removed && let Some(idle_gc) = &idle_gc {
                idle_gc.remove_agent(agent_id).await;
            }
            return;
        }
        let ended_at: Option<i64> = row.map(|r| r.get("ended_at"));
        if ended_at.is_some() {
            let removed = {
                let mut guard = inner.write().await;
                if Self::handle_matches_session(guard.get(agent_id), session_id) {
                    guard.remove(agent_id);
                    true
                } else {
                    false
                }
            };
            if removed && let Some(idle_gc) = &idle_gc {
                idle_gc.remove_agent(agent_id).await;
            }
            return;
        }

        let now = Utc::now().timestamp();
        let state = if success { "completed" } else { "failed" };
        let status = if success { "stopped" } else { "failed" };
        if let Err(err) = sqlx::query(
            r#"
            UPDATE agent_sessions
            SET status = ?1, ended_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(state)
        .bind(now)
        .bind(session_id)
        .execute(db)
        .await
        {
            tracing::warn!(
                agent_id = %agent_id,
                session_id = %session_id,
                status = %state,
                error = %err,
                "finalize_process_exit failed to update agent_sessions state"
            );
        }

        if let Err(err) = sqlx::query(
            r#"
            UPDATE agents
            SET status = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(status)
        .bind(now)
        .bind(agent_id)
        .execute(db)
        .await
        {
            tracing::warn!(
                agent_id = %agent_id,
                session_id = %session_id,
                status = %status,
                error = %err,
                "finalize_process_exit failed to update agents state"
            );
        }

        let seq = Uuid::now_v7().to_string();
        let ts = Utc::now().timestamp();
        let message = serde_json::json!({
            "type": "run_status",
            "status": state,
            "session_id": session_id,
        })
        .to_string();
        let event_id = match persist_agent_event(
            event_dbs,
            None,
            agent_id,
            session_id,
            &seq,
            ts,
            &OutputStream::Acp,
            &message,
        )
        .await
        {
            Ok(event_id) => event_id,
            Err(err) => {
                tracing::error!(
                    agent_id = %agent_id,
                    session_id = %session_id,
                    status = %state,
                    error = %err,
                    "finalize_process_exit failed to persist run-status event"
                );
                let mut guard = inner.write().await;
                let removed = if Self::handle_matches_session(guard.get(agent_id), session_id) {
                    guard.remove(agent_id);
                    true
                } else {
                    false
                };
                if removed && let Some(idle_gc) = &idle_gc {
                    idle_gc.remove_agent(agent_id).await;
                }
                return;
            }
        };
        let output = AgentOutput {
            event_id,
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            seq,
            ts,
            stream: OutputStream::Acp,
            message,
        };
        if let Some(handle) = inner
            .read()
            .await
            .get(agent_id)
            .filter(|handle| handle.session_id == session_id)
        {
            let _ = handle.output_tx.send(output);
        }

        if let Err(err) = push.notify_agent_completed(agent_id, session_id).await {
            tracing::warn!(
                agent_id = %agent_id,
                session_id = %session_id,
                error = %err,
                "finalize_process_exit failed to emit push completion notification"
            );
        }
        let mut guard = inner.write().await;
        let removed = if Self::handle_matches_session(guard.get(agent_id), session_id) {
            guard.remove(agent_id);
            true
        } else {
            false
        };
        if removed && let Some(idle_gc) = &idle_gc {
            idle_gc.remove_agent(agent_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use agenthub_db::AgentEventDbRouter;
    use chrono::Utc;
    use sqlx::{Row, SqlitePool};
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;
    use tokio::sync::{Mutex, broadcast};
    use tokio::time::{Duration, timeout};
    use uuid::Uuid;

    use crate::agent::AgentOutput;
    use crate::agent::OutputStream;
    use crate::agent::manager::{AgentHandle, AgentInput};

    async fn insert_agent_and_session(db: &SqlitePool, suffix: &str) -> (String, String) {
        let now = Utc::now().timestamp();
        let agent_id = format!("agent-process-{suffix}-{}", Uuid::new_v4());
        let session_id = format!("session-process-{suffix}-{}", Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, 0, NULL, NULL, ?7, ?8, ?9)
            "#,
        )
        .bind(&agent_id)
        .bind(format!("process-{suffix}"))
        .bind("/tmp")
        .bind("cat")
        .bind("[]")
        .bind("use_existing")
        .bind("running")
        .bind(now)
        .bind(now)
        .execute(db)
        .await
        .expect("insert test agent");
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind(&session_id)
        .bind(&agent_id)
        .bind(now)
        .execute(db)
        .await
        .expect("insert test session");
        (agent_id, session_id)
    }

    async fn collect_streams_for_lines(detect_acp_messages: bool, lines: &[&str]) -> Vec<String> {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) = insert_agent_and_session(&state.db, "stream-classify").await;
        let (mut writer, reader) = tokio::io::duplex(4096);
        let (output_tx, mut output_rx) = tokio::sync::broadcast::channel((lines.len() + 1).max(1));

        state
            .agents
            .spawn_output_reader(
                agent_id.clone(),
                session_id.clone(),
                OutputStream::Stderr,
                reader,
                output_tx,
                detect_acp_messages,
            )
            .await;

        for line in lines {
            writer.write_all(line.as_bytes()).await.expect("write line");
            writer.write_all(b"\n").await.expect("write newline");
        }
        writer.shutdown().await.expect("shutdown writer");
        drop(writer);

        for _ in 0..lines.len() {
            timeout(Duration::from_secs(2), output_rx.recv())
                .await
                .expect("timed out waiting for output reader to broadcast event")
                .expect("broadcast channel closed unexpectedly");
        }
        let event_db = state
            .agents
            .event_dbs
            .pool_for_agent(&agent_id)
            .await
            .expect("open per-agent event db");
        sqlx::query(
            r#"
            SELECT stream
            FROM agent_events
            WHERE session_id = ?1
            ORDER BY id ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&event_db)
        .await
        .expect("list persisted streams")
        .into_iter()
        .map(|row| row.get::<String, _>("stream"))
        .collect()
    }

    #[tokio::test]
    async fn spawn_output_reader_promotes_latest_codex_event_types_for_acp_agents() {
        let streams = collect_streams_for_lines(
            true,
            &[
                r#"{"type":"plan","steps":[{"title":"Investigate"}]}"#,
                r#"{"type":"available_commands","commands":["/compact","/undo"]}"#,
                r#"{"type":"current_mode","current_mode_id":"code"}"#,
                r#"{"type":"run_status","status":"completed","session_id":"s-1"}"#,
                "plain stderr line",
            ],
        )
        .await;
        assert_eq!(streams, vec!["acp", "acp", "acp", "acp", "stderr"]);
    }

    #[tokio::test]
    async fn spawn_output_reader_keeps_stderr_for_non_acp_agents() {
        let streams = collect_streams_for_lines(
            false,
            &[
                r#"{"type":"plan","steps":[{"title":"Investigate"}]}"#,
                r#"{"type":"run_status","status":"completed","session_id":"s-1"}"#,
            ],
        )
        .await;
        assert_eq!(streams, vec!["stderr", "stderr"]);
    }

    #[tokio::test]
    async fn finalize_process_exit_tolerates_closed_pool() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) = insert_agent_and_session(&state.db, "finalize-closed").await;
        state.db.close().await;

        super::AgentManager::finalize_process_exit(
            &state.db,
            &state.agents.event_dbs,
            state.agents.idle_gc.clone(),
            &state.agents.inner,
            &state.push,
            &agent_id,
            &session_id,
            false,
        )
        .await;
    }

    #[tokio::test]
    async fn finalize_process_exit_skips_event_persist_when_session_row_is_missing() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) =
            insert_agent_and_session(&state.db, "finalize-missing-session").await;
        let event_db_path = state.agents.event_dbs.db_path_for_agent(&agent_id);

        sqlx::query("DELETE FROM agent_sessions WHERE id = ?1")
            .bind(&session_id)
            .execute(&state.db)
            .await
            .expect("delete session row");

        assert!(
            !tokio::fs::try_exists(&event_db_path)
                .await
                .expect("check event db path before finalize"),
            "per-agent event db should not exist before finalize"
        );

        super::AgentManager::finalize_process_exit(
            &state.db,
            &state.agents.event_dbs,
            state.agents.idle_gc.clone(),
            &state.agents.inner,
            &state.push,
            &agent_id,
            &session_id,
            false,
        )
        .await;

        assert!(
            !tokio::fs::try_exists(&event_db_path)
                .await
                .expect("check event db path after finalize"),
            "finalize_process_exit must not recreate a deleted per-agent event db when the session row is already gone"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn finalize_process_exit_keeps_newer_running_handle_for_different_session() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, old_session_id) =
            insert_agent_and_session(&state.db, "finalize-mismatch-old").await;
        let now = Utc::now().timestamp();
        let new_session_id = format!("session-process-finalize-new-{}", Uuid::new_v4());

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind(&new_session_id)
        .bind(&agent_id)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert newer session");

        sqlx::query(
            r#"
            UPDATE agent_sessions
            SET status = 'failed', ended_at = ?1
            WHERE id = ?2
            "#,
        )
        .bind(now)
        .bind(&old_session_id)
        .execute(&state.db)
        .await
        .expect("mark old session ended");

        let child = Command::new("sh")
            .arg("-lc")
            .arg("sleep 5")
            .spawn()
            .expect("spawn newer child");
        let (output_tx, _rx) = broadcast::channel::<AgentOutput>(8);
        let handle = AgentHandle {
            child: std::sync::Arc::new(Mutex::new(Some(child))),
            output_tx,
            input: AgentInput::Stdin(std::sync::Arc::new(Mutex::new(None))),
            session_id: new_session_id.clone(),
            actor_context: None,
            acp_prompt_delivery_policy: None,
            loop_controller: None,
        };
        state
            .agents
            .inner
            .write()
            .await
            .insert(agent_id.clone(), handle);

        super::AgentManager::finalize_process_exit(
            &state.db,
            &state.agents.event_dbs,
            state.agents.idle_gc.clone(),
            &state.agents.inner,
            &state.push,
            &agent_id,
            &old_session_id,
            false,
        )
        .await;

        let running_handle_session = state
            .agents
            .inner
            .read()
            .await
            .get(&agent_id)
            .map(|handle| handle.session_id.clone());
        assert_eq!(
            running_handle_session.as_deref(),
            Some(new_session_id.as_str())
        );

        if let Some(handle) = state.agents.inner.write().await.remove(&agent_id) {
            let mut child_guard = handle.child.lock().await;
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn finalize_process_exit_keeps_newer_handle_when_event_persist_fails() {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, old_session_id) =
            insert_agent_and_session(&state.db, "finalize-persist-fail-old").await;
        let now = Utc::now().timestamp();
        let new_session_id = format!("session-process-finalize-persist-new-{}", Uuid::new_v4());

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind(&new_session_id)
        .bind(&agent_id)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert newer session");

        let child = Command::new("sh")
            .arg("-lc")
            .arg("sleep 5")
            .spawn()
            .expect("spawn newer child");
        let (output_tx, _rx) = broadcast::channel::<AgentOutput>(8);
        let handle = AgentHandle {
            child: std::sync::Arc::new(Mutex::new(Some(child))),
            output_tx,
            input: AgentInput::Stdin(std::sync::Arc::new(Mutex::new(None))),
            session_id: new_session_id.clone(),
            actor_context: None,
            acp_prompt_delivery_policy: None,
            loop_controller: None,
        };
        state
            .agents
            .inner
            .write()
            .await
            .insert(agent_id.clone(), handle);

        let broken_router_root =
            std::env::temp_dir().join(format!("agenthub-event-router-file-{}", Uuid::new_v4()));
        std::fs::write(&broken_router_root, b"not-a-directory").expect("create router root file");
        let broken_router = AgentEventDbRouter::new(broken_router_root);

        super::AgentManager::finalize_process_exit(
            &state.db,
            &broken_router,
            state.agents.idle_gc.clone(),
            &state.agents.inner,
            &state.push,
            &agent_id,
            &old_session_id,
            false,
        )
        .await;

        let running_handle_session = state
            .agents
            .inner
            .read()
            .await
            .get(&agent_id)
            .map(|handle| handle.session_id.clone());
        assert_eq!(
            running_handle_session.as_deref(),
            Some(new_session_id.as_str())
        );

        if let Some(handle) = state.agents.inner.write().await.remove(&agent_id) {
            let mut child_guard = handle.child.lock().await;
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        }
    }
}
