use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;

use super::codec::{is_acp_message, is_dir_empty, stream_to_str};
use super::{AgentHandle, AgentManager};
use crate::agent::{AgentOutput, AgentRecord, OutputStream, WorktreeMode};
use crate::push::PushService;

impl AgentManager {
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
        let result = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&agent_id)
        .bind(&session_id)
        .bind(&seq)
        .bind(ts)
        .bind(stream_to_str(&OutputStream::Acp))
        .bind(message.clone())
        .execute(&self.db)
        .await;
        let Ok(result) = result else {
            tracing::error!("emit_run_status: failed to persist event");
            return;
        };
        let output = AgentOutput {
            event_id: result.last_insert_rowid(),
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
    ) where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let db = self.db.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let is_acp = is_acp_message(&line);
                let stream = if is_acp {
                    OutputStream::Acp
                } else {
                    stream.clone()
                };
                let seq = Uuid::now_v7().to_string();
                let ts = Utc::now().timestamp();
                let stream_name = stream_to_str(&stream);
                let result = sqlx::query(
                    r#"
                    INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                )
                .bind(&agent_id)
                .bind(&session_id)
                .bind(&seq)
                .bind(ts)
                .bind(stream_name)
                .bind(&line)
                .execute(&db)
                .await;
                let Ok(result) = result else {
                    tracing::error!("spawn_output_reader: failed to persist event");
                    continue;
                };
                let output = AgentOutput {
                    event_id: result.last_insert_rowid(),
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
        let inner = self.inner.clone();
        let push = self.push.clone();
        let agent_id_clone = agent_id.clone();
        tokio::spawn(async move {
            let child = {
                let guard = inner.read().await;
                guard.get(&agent_id).map(|h| h.child.clone())
            };

            if let Some(child_mutex) = child {
                let success = {
                    let mut child_guard = child_mutex.lock().await;
                    let child = match child_guard.as_mut() {
                        Some(child) => child,
                        None => return,
                    };
                    match child.wait().await {
                        Ok(status) => status.success(),
                        Err(_) => false,
                    }
                };
                Self::finalize_process_exit(
                    &db,
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

    pub(super) async fn finalize_process_exit(
        db: &SqlitePool,
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
        .await
        .ok()
        .flatten();
        let ended_at: Option<i64> = row.map(|r| r.get("ended_at"));
        if ended_at.is_some() {
            let mut guard = inner.write().await;
            guard.remove(agent_id);
            return;
        }

        let now = Utc::now().timestamp();
        let state = if success { "completed" } else { "failed" };
        let status = if success { "stopped" } else { "failed" };
        let _ = sqlx::query(
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
        .await;

        let _ = sqlx::query(
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
        .await;

        let seq = Uuid::now_v7().to_string();
        let ts = Utc::now().timestamp();
        let message = serde_json::json!({
            "type": "run_status",
            "status": state,
            "session_id": session_id,
        })
        .to_string();
        let result = sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(agent_id)
        .bind(session_id)
        .bind(&seq)
        .bind(ts)
        .bind(stream_to_str(&OutputStream::Acp))
        .bind(message.clone())
        .execute(db)
        .await;

        let Ok(result) = result else {
            tracing::error!("finalize_process_exit: failed to persist event");
            let mut guard = inner.write().await;
            guard.remove(agent_id);
            return;
        };

        let output = AgentOutput {
            event_id: result.last_insert_rowid(),
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            seq,
            ts,
            stream: OutputStream::Acp,
            message,
        };
        if let Some(handle) = inner.read().await.get(agent_id) {
            let _ = handle.output_tx.send(output);
        }

        let _ = push.notify_agent_completed(agent_id, session_id).await;
        let mut guard = inner.write().await;
        guard.remove(agent_id);
    }

    pub(super) async fn prepare_worktree_with_paths(
        &self,
        agent: &AgentRecord,
        workdir: &str,
        worktree_repo: Option<&str>,
    ) -> anyhow::Result<()> {
        match agent.worktree_mode {
            WorktreeMode::UseExisting => Ok(()),
            WorktreeMode::ReuseWorktree => {
                if !Path::new(workdir).exists() {
                    let detail = format!(
                        "agent_id={}, mode=reuse_worktree, workdir={}, error=worktree missing",
                        agent.id, workdir
                    );
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_reuse_missing",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    anyhow::bail!("worktree does not exist");
                }
                let detail = format!(
                    "agent_id={}, mode=reuse_worktree, workdir={}",
                    agent.id, workdir
                );
                let _ = self
                    .auth
                    .record_audit(None, None, "worktree_reuse", Some(&detail), None, None)
                    .await;
                Ok(())
            }
            WorktreeMode::CreateWorktree => {
                let repo =
                    worktree_repo.ok_or_else(|| anyhow::anyhow!("worktree_repo required"))?;
                if let Err(err) = self.ensure_safe_path(repo).await {
                    let detail = format!(
                        "agent_id={}, mode=create_worktree, repo={}, workdir={}, error={}",
                        agent.id, repo, workdir, err
                    );
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_create_failed",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    return Err(err);
                }
                let workdir_path = Path::new(workdir);
                if workdir_path.exists() && !is_dir_empty(workdir_path)? {
                    let detail = format!(
                        "agent_id={}, mode=create_worktree, repo={}, workdir={}, error=workdir not empty",
                        agent.id, repo, workdir
                    );
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_create_failed",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    anyhow::bail!("workdir is not empty");
                }
                let ref_name = agent.worktree_ref.as_deref().unwrap_or("HEAD");
                let output = Command::new("git")
                    .arg("-C")
                    .arg(repo)
                    .arg("worktree")
                    .arg("add")
                    .arg(workdir)
                    .arg(ref_name)
                    .output()
                    .await?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let detail = format!(
                        "agent_id={}, mode=create_worktree, repo={}, workdir={}, ref={}, error={}",
                        agent.id,
                        repo,
                        workdir,
                        ref_name,
                        stderr.trim()
                    );
                    let _ = self
                        .auth
                        .record_audit(
                            None,
                            None,
                            "worktree_create_failed",
                            Some(&detail),
                            None,
                            None,
                        )
                        .await;
                    anyhow::bail!("git worktree add failed: {}", stderr.trim());
                }
                let detail = format!(
                    "agent_id={}, mode=create_worktree, repo={}, workdir={}, ref={}",
                    agent.id, repo, workdir, ref_name
                );
                let _ = self
                    .auth
                    .record_audit(None, None, "worktree_created", Some(&detail), None, None)
                    .await;
                Ok(())
            }
        }
    }

    pub async fn set_code_mode(&self, agent_id: &str, code_mode: bool) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE agents
            SET code_mode = ?1, updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(if code_mode { 1 } else { 0 })
        .bind(now)
        .bind(agent_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn mark_exited_on_startup(&self) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let _ = sqlx::query(
            r#"
            UPDATE agent_sessions
            SET status = 'exited', ended_at = ?1
            WHERE status = 'running' AND ended_at IS NULL
            "#,
        )
        .bind(now)
        .execute(&self.db)
        .await?;

        let _ = sqlx::query(
            r#"
            UPDATE agents
            SET status = 'exited', updated_at = ?1
            WHERE status = 'running'
            "#,
        )
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn delete_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let mut guard = self.inner.write().await;
        guard.remove(agent_id);
        drop(guard);
        let mut tx = self.db.begin().await?;
        sqlx::query("DELETE FROM acp_permission_requests WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agent_events WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agent_sessions WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agent_persistent_sessions WHERE agent_id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM agents WHERE id = ?1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
