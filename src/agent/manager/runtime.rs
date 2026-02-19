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
use super::{AgentHandle, AgentManager, normalize_path};
use crate::agent::{AgentOutput, AgentRecord, OutputStream, WorktreeMode};
use crate::push::PushService;

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitWorktreeEntry {
    path: String,
    head: Option<String>,
    branch: Option<String>,
}

async fn repo_find_worktree_entry(
    repo: &str,
    workdir: &str,
) -> anyhow::Result<Option<GitWorktreeEntry>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .output()
        .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.trim();
        if reason.is_empty() {
            anyhow::bail!("git worktree list failed");
        }
        anyhow::bail!("git worktree list failed: {reason}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let target = normalize_worktree_path(workdir);
    Ok(parse_worktree_list(&stdout)
        .into_iter()
        .find(|entry| normalize_worktree_path(&entry.path) == target))
}

fn parse_worktree_list(stdout: &str) -> Vec<GitWorktreeEntry> {
    let mut entries = Vec::new();
    let mut current: Option<GitWorktreeEntry> = None;
    for line in stdout.lines() {
        if line.is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(GitWorktreeEntry {
                path: path.to_string(),
                head: None,
                branch: None,
            });
            continue;
        }
        if let Some(head) = line.strip_prefix("HEAD ") {
            if let Some(entry) = current.as_mut() {
                entry.head = Some(head.to_string());
            }
            continue;
        }
        if let Some(branch) = line.strip_prefix("branch ")
            && let Some(entry) = current.as_mut()
        {
            entry.branch = Some(branch.to_string());
        }
    }
    if let Some(entry) = current.take() {
        entries.push(entry);
    }
    entries
}

fn trim_ref_prefix(value: &str) -> &str {
    value.strip_prefix("refs/heads/").unwrap_or(value)
}

fn is_hex_sha(value: &str) -> bool {
    let len = value.len();
    (7..=64).contains(&len) && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn worktree_ref_matches(entry: &GitWorktreeEntry, expected_ref: &str) -> bool {
    let expected = expected_ref.trim();
    if expected.eq_ignore_ascii_case("HEAD") {
        return true;
    }

    let expected_branch = trim_ref_prefix(expected);
    if !expected_branch.is_empty()
        && let Some(branch) = entry.branch.as_deref()
        && trim_ref_prefix(branch) == expected_branch
    {
        return true;
    }

    if is_hex_sha(expected)
        && let Some(head) = entry.head.as_deref()
    {
        return head.starts_with(expected);
    }

    false
}

fn normalize_worktree_path(path: &str) -> String {
    let canonical = std::fs::canonicalize(path).or_else(|err| {
        tracing::warn!(
            path = %path,
            error = %err,
            "failed to canonicalize worktree path"
        );
        if Path::new(path).is_absolute() {
            return Ok(Path::new(path).to_path_buf());
        }
        std::env::current_dir().map(|cwd| cwd.join(path))
    });
    let canonical = canonical
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());
    normalize_path(&canonical)
}

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
        detect_acp_messages: bool,
    ) where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let db = self.db.clone();
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
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "reuse_worktree",
                        "workdir": workdir,
                        "error": "worktree missing",
                    })
                    .to_string();
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
                let detail = serde_json::json!({
                    "agent_id": agent.id,
                    "mode": "reuse_worktree",
                    "workdir": workdir,
                })
                .to_string();
                let _ = self
                    .auth
                    .record_audit(None, None, "worktree_reuse", Some(&detail), None, None)
                    .await;
                Ok(())
            }
            WorktreeMode::CreateWorktree => {
                let repo =
                    worktree_repo.ok_or_else(|| anyhow::anyhow!("worktree_repo required"))?;
                let ref_name = agent.worktree_ref.as_deref().unwrap_or("HEAD");
                if let Err(err) = self.ensure_safe_path(repo).await {
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "create_worktree",
                        "repo": repo,
                        "workdir": workdir,
                        "error": err.to_string(),
                    })
                    .to_string();
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
                if let Err(err) = self.ensure_safe_path(workdir).await {
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "create_worktree",
                        "repo": repo,
                        "workdir": workdir,
                        "error": err.to_string(),
                    })
                    .to_string();
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
                    let existing_worktree = match repo_find_worktree_entry(repo, workdir).await {
                        Ok(entry) => entry,
                        Err(err) => {
                            let detail = serde_json::json!({
                                "agent_id": agent.id,
                                "mode": "create_worktree",
                                "repo": repo,
                                "workdir": workdir,
                                "error": format!("worktree list failed: {err}"),
                            })
                            .to_string();
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
                    };
                    if let Some(existing_worktree) = existing_worktree {
                        if self
                            .is_workdir_bound_to_other_agent(&agent.id, workdir)
                            .await?
                        {
                            let detail = serde_json::json!({
                                "agent_id": agent.id,
                                "mode": "create_worktree",
                                "repo": repo,
                                "workdir": workdir,
                                "error": "workdir belongs to another agent",
                            })
                            .to_string();
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
                            anyhow::bail!("existing worktree belongs to another agent");
                        }

                        if !worktree_ref_matches(&existing_worktree, ref_name) {
                            let existing_branch = existing_worktree.branch.as_deref();
                            let existing_head = existing_worktree.head.as_deref();
                            let detail = serde_json::json!({
                                "agent_id": agent.id,
                                "mode": "create_worktree",
                                "repo": repo,
                                "workdir": workdir,
                                "configured_ref": ref_name,
                                "existing_branch": existing_branch,
                                "existing_head": existing_head,
                                "action": "reuse_existing_worktree",
                            })
                            .to_string();
                            let _ = self
                                .auth
                                .record_audit(
                                    None,
                                    None,
                                    "worktree_reuse_ref_mismatch",
                                    Some(&detail),
                                    None,
                                    None,
                                )
                                .await;
                            tracing::warn!(
                                agent_id = %agent.id,
                                repo = %repo,
                                workdir = %workdir,
                                configured_ref = %ref_name,
                                existing_branch = ?existing_branch,
                                existing_head = ?existing_head,
                                "existing worktree ref mismatched configured ref; reusing worktree"
                            );
                        }

                        let detail = serde_json::json!({
                            "agent_id": agent.id,
                            "mode": "create_worktree",
                            "repo": repo,
                            "workdir": workdir,
                            "source": "existing_worktree",
                        })
                        .to_string();
                        let _ = self
                            .auth
                            .record_audit(None, None, "worktree_reuse", Some(&detail), None, None)
                            .await;
                        return Ok(());
                    }
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "create_worktree",
                        "repo": repo,
                        "workdir": workdir,
                        "error": "workdir not empty",
                    })
                    .to_string();
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
                    let detail = serde_json::json!({
                        "agent_id": agent.id,
                        "mode": "create_worktree",
                        "repo": repo,
                        "workdir": workdir,
                        "ref": ref_name,
                        "error": stderr.trim(),
                    })
                    .to_string();
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
                let detail = serde_json::json!({
                    "agent_id": agent.id,
                    "mode": "create_worktree",
                    "repo": repo,
                    "workdir": workdir,
                    "ref": ref_name,
                })
                .to_string();
                let _ = self
                    .auth
                    .record_audit(None, None, "worktree_created", Some(&detail), None, None)
                    .await;
                Ok(())
            }
        }
    }

    async fn is_workdir_bound_to_other_agent(
        &self,
        agent_id: &str,
        workdir: &str,
    ) -> anyhow::Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT id
            FROM agents
            WHERE workdir = ?1 AND id != ?2
            LIMIT 1
            "#,
        )
        .bind(workdir)
        .bind(agent_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.is_some())
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

#[cfg(test)]
mod tests {
    use super::{GitWorktreeEntry, is_hex_sha, parse_worktree_list, worktree_ref_matches};
    use crate::agent::OutputStream;
    use chrono::Utc;
    use sqlx::{Row, SqlitePool};
    use tokio::io::AsyncWriteExt;
    use tokio::time::{Duration, sleep, timeout};
    use uuid::Uuid;

    async fn insert_agent_and_session(db: &SqlitePool, suffix: &str) -> (String, String) {
        let now = Utc::now().timestamp();
        let agent_id = format!("agent-runtime-{suffix}-{}", Uuid::new_v4());
        let session_id = format!("session-runtime-{suffix}-{}", Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 0, ?7, ?8, ?9)
            "#,
        )
        .bind(&agent_id)
        .bind(format!("runtime-{suffix}"))
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

    async fn wait_for_event_count(
        db: &SqlitePool,
        agent_id: &str,
        session_id: &str,
        expected: i64,
    ) {
        timeout(Duration::from_secs(2), async {
            loop {
                let count: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*)
                    FROM agent_events
                    WHERE agent_id = ?1 AND session_id = ?2
                    "#,
                )
                .bind(agent_id)
                .bind(session_id)
                .fetch_one(db)
                .await
                .expect("count events");
                if count >= expected {
                    return;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("timed out waiting for output reader to persist events");
    }

    async fn collect_streams_for_lines(detect_acp_messages: bool, lines: &[&str]) -> Vec<String> {
        let state = crate::api::team_tests::build_test_state().await;
        let (agent_id, session_id) = insert_agent_and_session(&state.db, "stream-classify").await;
        let (mut writer, reader) = tokio::io::duplex(4096);
        let (output_tx, _output_rx) = tokio::sync::broadcast::channel(32);

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

        wait_for_event_count(&state.db, &agent_id, &session_id, lines.len() as i64).await;
        sqlx::query(
            r#"
            SELECT stream
            FROM agent_events
            WHERE agent_id = ?1 AND session_id = ?2
            ORDER BY id ASC
            "#,
        )
        .bind(agent_id)
        .bind(session_id)
        .fetch_all(&state.db)
        .await
        .expect("list persisted streams")
        .into_iter()
        .map(|row| row.get::<String, _>("stream"))
        .collect()
    }

    #[test]
    fn parse_worktree_list_extracts_entries() {
        let stdout = r#"
worktree /tmp/repo
HEAD 0000000000000000000000000000000000000000
branch refs/heads/main

worktree /tmp/repo/worktrees/agent-a
HEAD 1111111111111111111111111111111111111111
branch refs/heads/agent-a
"#;
        let entries = parse_worktree_list(stdout);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].path, "/tmp/repo/worktrees/agent-a");
        assert_eq!(entries[1].branch.as_deref(), Some("refs/heads/agent-a"));
    }

    #[test]
    fn worktree_ref_matches_accepts_head() {
        let entry = GitWorktreeEntry {
            path: "/tmp/repo/worktrees/agent-a".to_string(),
            head: Some("1111111111111111111111111111111111111111".to_string()),
            branch: Some("refs/heads/agent-a".to_string()),
        };
        assert!(worktree_ref_matches(&entry, "HEAD"));
    }

    #[test]
    fn worktree_ref_matches_accepts_matching_branch() {
        let entry = GitWorktreeEntry {
            path: "/tmp/repo/worktrees/agent-a".to_string(),
            head: Some("1111111111111111111111111111111111111111".to_string()),
            branch: Some("refs/heads/agent-a".to_string()),
        };
        assert!(worktree_ref_matches(&entry, "refs/heads/agent-a"));
        assert!(worktree_ref_matches(&entry, "agent-a"));
        assert!(!worktree_ref_matches(&entry, "agent-b"));
    }

    #[test]
    fn worktree_ref_matches_accepts_matching_commit_prefix() {
        let entry = GitWorktreeEntry {
            path: "/tmp/repo/worktrees/agent-a".to_string(),
            head: Some("1111111111111111111111111111111111111111".to_string()),
            branch: None,
        };
        assert!(worktree_ref_matches(&entry, "1111111"));
        assert!(!worktree_ref_matches(&entry, "2222222"));
    }

    #[test]
    fn is_hex_sha_accepts_sha256_length() {
        assert!(is_hex_sha(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
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
}
