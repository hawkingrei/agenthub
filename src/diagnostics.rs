#[cfg(debug_assertions)]
pub(crate) mod agent_trace {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use anyhow::{Context, bail};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use sqlx::{
        Row, SqlitePool,
        sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    };

    const DEFAULT_EVENT_LIMIT: i64 = 16;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct AgentTraceRequest {
        pub agent_id: Option<String>,
        pub team_id: Option<String>,
        pub member_id: Option<String>,
        pub session_id: Option<String>,
        pub event_limit: i64,
    }

    impl AgentTraceRequest {
        pub(crate) fn normalize_limit(&self) -> i64 {
            self.event_limit.clamp(1, 100)
        }

        pub(crate) fn validate(&self) -> anyhow::Result<()> {
            let has_agent = self
                .agent_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            let has_team = self
                .team_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            let has_member = self
                .member_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            match (has_agent, has_team, has_member) {
                (true, false, false) => Ok(()),
                (false, true, true) => Ok(()),
                (true, _, _) => bail!(
                    "--agent-id cannot be combined with --team-id or --member-id for agent-trace"
                ),
                _ => bail!(
                    "agent-trace requires either --agent-id or both --team-id and --member-id"
                ),
            }
        }
    }

    impl Default for AgentTraceRequest {
        fn default() -> Self {
            Self {
                agent_id: None,
                team_id: None,
                member_id: None,
                session_id: None,
                event_limit: DEFAULT_EVENT_LIMIT,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceLiveOverlay {
        pub runtime: AgentTraceRuntimeSummary,
        pub provider_adapter: AgentTraceAvailability,
        pub sse: AgentTraceAvailability,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceReport {
        pub build: AgentTraceBuild,
        pub target: AgentTraceTarget,
        pub agent: Option<AgentTraceAgent>,
        pub team: Option<AgentTraceTeam>,
        pub runtime: AgentTraceRuntimeSummary,
        pub session: Option<AgentTraceSession>,
        pub events: AgentTraceEventSummary,
        pub permissions: AgentTracePermissionSummary,
        pub mailbox: AgentTraceMailboxSummary,
        pub provider_adapter: AgentTraceAvailability,
        pub sse: AgentTraceAvailability,
        pub verdict: AgentTraceVerdict,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceBuild {
        pub diagnostics_enabled: bool,
        pub debug_assertions: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceTarget {
        pub agent_id: String,
        pub team_id: Option<String>,
        pub member_id: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceAgent {
        pub id: String,
        pub name: String,
        pub status: String,
        pub target_node_id: Option<String>,
        pub updated_at: i64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceTeam {
        pub id: String,
        pub member_found: bool,
        pub latest_run_id: Option<String>,
        pub latest_run_status: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceSession {
        pub id: String,
        pub status: String,
        pub started_at: i64,
        pub ended_at: Option<i64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceRuntimeSummary {
        pub ownership: String,
        pub active_session_id: Option<String>,
        pub live_state_source: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceAvailability {
        pub status: String,
        pub note: String,
        #[serde(default, skip_serializing_if = "Value::is_null")]
        pub details: Value,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceEventSummary {
        pub event_db_path: String,
        pub event_db_exists: bool,
        pub count: usize,
        pub latest: Option<AgentTraceEvent>,
        pub recent: Vec<AgentTraceEvent>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceEvent {
        pub event_id: i64,
        pub session_id: String,
        pub seq: String,
        pub ts: i64,
        pub stream: String,
        pub event_type: Option<String>,
        pub status: Option<String>,
        pub tool_call_id: Option<String>,
        pub permission_id: Option<String>,
        pub redacted_fields: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTracePermissionSummary {
        pub pending_count: i64,
        pub pending_tool_call_ids: Vec<String>,
        pub pending_permission_ids: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceMailboxSummary {
        pub pending_to_actor_count: i64,
        pub latest_pending_message_id: Option<i64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct AgentTraceVerdict {
        pub layer: AgentTraceStallLayer,
        pub reason: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub(crate) enum AgentTraceStallLayer {
        TargetNotFound,
        TeamMemberNotFound,
        RuntimeNotRunning,
        WaitingPermission,
        ProviderPromptStale,
        MailboxPending,
        NoPersistedEvents,
        EventStreamPresent,
    }

    pub(crate) async fn collect_from_default_paths(
        request: AgentTraceRequest,
    ) -> anyhow::Result<AgentTraceReport> {
        request.validate()?;
        let db_path = agenthub_db::default_db_path();
        if !db_path.exists() {
            bail!("agenthub database does not exist: {}", db_path.display());
        }
        let db = open_read_only_sqlite(&db_path)
            .await
            .with_context(|| format!("open agenthub database {}", db_path.display()))?;
        collect_from_pool(&db, agenthub_db::default_agent_event_db_dir(), request).await
    }

    pub(crate) async fn collect_from_pool(
        db: &SqlitePool,
        event_db_dir: PathBuf,
        request: AgentTraceRequest,
    ) -> anyhow::Result<AgentTraceReport> {
        request.validate()?;
        let resolved = resolve_target(db, &request).await?;
        let agent = load_agent(db, &resolved.agent_id).await?;
        let session = load_session(db, &resolved.agent_id, request.session_id.as_deref()).await?;
        let session_id = session.as_ref().map(|session| session.id.as_str());
        let events = load_events(
            &event_db_dir,
            &resolved.agent_id,
            session_id,
            request.normalize_limit(),
        )
        .await?;
        let permissions = load_pending_permissions(db, &resolved.agent_id, session_id).await?;
        let mailbox =
            load_mailbox_summary(db, &resolved.agent_id, resolved.team_id.as_deref()).await?;
        let verdict = classify_stall(
            &agent,
            &resolved.team,
            &session,
            &events,
            &permissions,
            &mailbox,
        );

        Ok(AgentTraceReport {
            build: AgentTraceBuild {
                diagnostics_enabled: true,
                debug_assertions: true,
            },
            target: resolved.target,
            runtime: runtime_summary(&agent, &session),
            agent,
            team: resolved.team,
            session,
            events,
            permissions,
            mailbox,
            provider_adapter: AgentTraceAvailability {
                status: "unavailable_in_db_snapshot".to_string(),
                note: "provider adapter progress requires a live backend diagnostic path"
                    .to_string(),
                details: Value::Null,
            },
            sse: AgentTraceAvailability {
                status: "unavailable_in_db_snapshot".to_string(),
                note: "SSE broadcaster freshness requires a live backend diagnostic path"
                    .to_string(),
                details: Value::Null,
            },
            verdict,
        })
    }

    pub(crate) fn apply_live_overlay(
        report: &mut AgentTraceReport,
        overlay: AgentTraceLiveOverlay,
    ) {
        report.runtime = overlay.runtime;
        report.provider_adapter = overlay.provider_adapter;
        report.sse = overlay.sse;
        if report.provider_adapter.status == "prompt_stale" {
            report.verdict = AgentTraceVerdict {
                layer: AgentTraceStallLayer::ProviderPromptStale,
                reason: "live provider adapter has an active prompt without recent provider events or pending permission".to_string(),
            };
        }
    }

    pub(crate) fn render_human(report: &AgentTraceReport) -> String {
        let mut lines = vec![
            "Agent trace diagnostics (debug build only)".to_string(),
            format!("target.agent_id: {}", report.target.agent_id),
        ];
        if let Some(team_id) = &report.target.team_id {
            lines.push(format!("target.team_id: {team_id}"));
        }
        if let Some(member_id) = &report.target.member_id {
            lines.push(format!("target.member_id: {member_id}"));
        }
        if let Some(agent) = &report.agent {
            lines.push(format!("agent.status: {}", agent.status));
            lines.push(format!(
                "agent.target_node_id: {}",
                agent.target_node_id.as_deref().unwrap_or("<local>")
            ));
        } else {
            lines.push("agent: <missing>".to_string());
        }
        if let Some(team) = &report.team {
            lines.push(format!("team.member_found: {}", team.member_found));
            lines.push(format!(
                "team.latest_run: {} ({})",
                team.latest_run_id.as_deref().unwrap_or("<none>"),
                team.latest_run_status.as_deref().unwrap_or("unknown")
            ));
        }
        if let Some(session) = &report.session {
            lines.push(format!("session.id: {}", session.id));
            lines.push(format!("session.status: {}", session.status));
        } else {
            lines.push("session: <none>".to_string());
        }
        lines.push(format!("runtime.ownership: {}", report.runtime.ownership));
        lines.push(format!(
            "runtime.live_state_source: {}",
            report.runtime.live_state_source
        ));
        lines.push(format!(
            "provider_adapter.status: {}",
            report.provider_adapter.status
        ));
        lines.push(format!("sse.status: {}", report.sse.status));
        lines.push(format!(
            "events.db: {} ({})",
            report.events.event_db_path,
            if report.events.event_db_exists {
                "exists"
            } else {
                "missing"
            }
        ));
        lines.push(format!("events.count: {}", report.events.count));
        if let Some(event) = &report.events.latest {
            lines.push(format!(
                "events.latest: id={} ts={} stream={} type={} status={} tool_call_id={} permission_id={}",
                event.event_id,
                event.ts,
                event.stream,
                event.event_type.as_deref().unwrap_or("<unknown>"),
                event.status.as_deref().unwrap_or("<unknown>"),
                event.tool_call_id.as_deref().unwrap_or("<none>"),
                event.permission_id.as_deref().unwrap_or("<none>")
            ));
        }
        lines.push(format!(
            "permissions.pending_count: {}",
            report.permissions.pending_count
        ));
        if !report.permissions.pending_permission_ids.is_empty() {
            lines.push(format!(
                "permissions.pending_ids: {}",
                report.permissions.pending_permission_ids.join(", ")
            ));
        }
        lines.push(format!(
            "mailbox.pending_to_actor_count: {}",
            report.mailbox.pending_to_actor_count
        ));
        lines.push(format!("verdict.layer: {:?}", report.verdict.layer));
        lines.push(format!("verdict.reason: {}", report.verdict.reason));
        lines.join("\n")
    }

    async fn open_read_only_sqlite(path: &Path) -> anyhow::Result<SqlitePool> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .read_only(true)
            .busy_timeout(Duration::from_secs(5));
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(Into::into)
    }

    struct ResolvedTarget {
        target: AgentTraceTarget,
        team: Option<AgentTraceTeam>,
        agent_id: String,
        team_id: Option<String>,
    }

    async fn resolve_target(
        db: &SqlitePool,
        request: &AgentTraceRequest,
    ) -> anyhow::Result<ResolvedTarget> {
        if let Some(agent_id) = request
            .agent_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(ResolvedTarget {
                target: AgentTraceTarget {
                    agent_id: agent_id.to_string(),
                    team_id: None,
                    member_id: None,
                },
                team: None,
                agent_id: agent_id.to_string(),
                team_id: None,
            });
        }

        let team_id = request
            .team_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("missing team id")?;
        let member_id = request
            .member_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("missing member id")?;
        let team = load_team(db, team_id, member_id).await?;
        Ok(ResolvedTarget {
            target: AgentTraceTarget {
                agent_id: member_id.to_string(),
                team_id: Some(team_id.to_string()),
                member_id: Some(member_id.to_string()),
            },
            team: Some(team),
            agent_id: member_id.to_string(),
            team_id: Some(team_id.to_string()),
        })
    }

    async fn load_team(
        db: &SqlitePool,
        team_id: &str,
        member_id: &str,
    ) -> anyhow::Result<AgentTraceTeam> {
        let row = sqlx::query(
            r#"
            SELECT spec_json
            FROM team_definitions
            WHERE id = ?1
            LIMIT 1
            "#,
        )
        .bind(team_id)
        .fetch_optional(db)
        .await?;
        let member_found = row
            .and_then(|row| row.try_get::<String, _>("spec_json").ok())
            .and_then(|spec| serde_json::from_str::<Value>(&spec).ok())
            .is_some_and(|spec| {
                spec.get("members")
                    .and_then(Value::as_array)
                    .is_some_and(|members| {
                        members.iter().any(|member| {
                            member
                                .get("member_id")
                                .and_then(Value::as_str)
                                .is_some_and(|id| id == member_id)
                        })
                    })
            });

        let run = sqlx::query(
            r#"
            SELECT id, status
            FROM team_runs
            WHERE team_id = ?1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(team_id)
        .fetch_optional(db)
        .await?;

        Ok(AgentTraceTeam {
            id: team_id.to_string(),
            member_found,
            latest_run_id: run
                .as_ref()
                .and_then(|row| row.try_get::<String, _>("id").ok()),
            latest_run_status: run
                .as_ref()
                .and_then(|row| row.try_get::<String, _>("status").ok()),
        })
    }

    async fn load_agent(
        db: &SqlitePool,
        agent_id: &str,
    ) -> anyhow::Result<Option<AgentTraceAgent>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, status, target_node_id, updated_at
            FROM agents
            WHERE id = ?1
            LIMIT 1
            "#,
        )
        .bind(agent_id)
        .fetch_optional(db)
        .await?;
        Ok(row.map(|row| AgentTraceAgent {
            id: row.get("id"),
            name: row.get("name"),
            status: row.get("status"),
            target_node_id: row
                .try_get::<Option<String>, _>("target_node_id")
                .ok()
                .flatten(),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn load_session(
        db: &SqlitePool,
        agent_id: &str,
        requested_session_id: Option<&str>,
    ) -> anyhow::Result<Option<AgentTraceSession>> {
        let row = if let Some(session_id) = requested_session_id {
            sqlx::query(
                r#"
                SELECT id, status, started_at, ended_at
                FROM agent_sessions
                WHERE id = ?1 AND agent_id = ?2
                LIMIT 1
                "#,
            )
            .bind(session_id)
            .bind(agent_id)
            .fetch_optional(db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, status, started_at, ended_at
                FROM agent_sessions
                WHERE agent_id = ?1
                ORDER BY started_at DESC
                LIMIT 1
                "#,
            )
            .bind(agent_id)
            .fetch_optional(db)
            .await?
        };
        Ok(row.map(|row| AgentTraceSession {
            id: row.get("id"),
            status: row.get("status"),
            started_at: row.get("started_at"),
            ended_at: row.try_get::<Option<i64>, _>("ended_at").ok().flatten(),
        }))
    }

    async fn load_events(
        event_db_dir: &Path,
        agent_id: &str,
        session_id: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<AgentTraceEventSummary> {
        let event_db_path = event_db_dir.join(format!("{agent_id}.db"));
        if !event_db_path.exists() {
            return Ok(AgentTraceEventSummary {
                event_db_path: event_db_path.display().to_string(),
                event_db_exists: false,
                count: 0,
                latest: None,
                recent: vec![],
            });
        }
        let event_db = open_read_only_sqlite(&event_db_path)
            .await
            .with_context(|| format!("open agent event db {}", event_db_path.display()))?;
        let rows = if let Some(session_id) = session_id {
            sqlx::query(
                r#"
                SELECT id, session_id, seq, ts, stream, message
                FROM agent_events
                WHERE session_id = ?1
                ORDER BY id DESC
                LIMIT ?2
                "#,
            )
            .bind(session_id)
            .bind(limit)
            .fetch_all(&event_db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, session_id, seq, ts, stream, message
                FROM agent_events
                ORDER BY id DESC
                LIMIT ?1
                "#,
            )
            .bind(limit)
            .fetch_all(&event_db)
            .await?
        };
        let mut recent = rows
            .into_iter()
            .map(|row| {
                let message = row
                    .try_get::<Vec<u8>, _>("message")
                    .ok()
                    .map(|bytes| {
                        crate::agent::event_message_codec::decode_message_from_storage(&bytes)
                    })
                    .unwrap_or_default();
                event_from_row(
                    row.get("id"),
                    row.get("session_id"),
                    row.get("seq"),
                    row.get("ts"),
                    row.get("stream"),
                    message.as_str(),
                )
            })
            .collect::<Vec<_>>();
        recent.reverse();
        Ok(AgentTraceEventSummary {
            event_db_path: event_db_path.display().to_string(),
            event_db_exists: true,
            count: recent.len(),
            latest: recent.last().cloned(),
            recent,
        })
    }

    async fn load_pending_permissions(
        db: &SqlitePool,
        agent_id: &str,
        session_id: Option<&str>,
    ) -> anyhow::Result<AgentTracePermissionSummary> {
        let rows = if let Some(session_id) = session_id {
            sqlx::query(
                r#"
                SELECT id, tool_call_id
                FROM acp_permission_requests
                WHERE agent_id = ?1 AND session_id = ?2 AND status = 'pending'
                ORDER BY created_at DESC
                LIMIT 20
                "#,
            )
            .bind(agent_id)
            .bind(session_id)
            .fetch_all(db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, tool_call_id
                FROM acp_permission_requests
                WHERE agent_id = ?1 AND status = 'pending'
                ORDER BY created_at DESC
                LIMIT 20
                "#,
            )
            .bind(agent_id)
            .fetch_all(db)
            .await?
        };
        let pending_count = rows.len() as i64;
        Ok(AgentTracePermissionSummary {
            pending_count,
            pending_tool_call_ids: rows
                .iter()
                .filter_map(|row| {
                    row.try_get::<Option<String>, _>("tool_call_id")
                        .ok()
                        .flatten()
                })
                .collect(),
            pending_permission_ids: rows
                .iter()
                .filter_map(|row| row.try_get::<String, _>("id").ok())
                .collect(),
        })
    }

    async fn load_mailbox_summary(
        db: &SqlitePool,
        agent_id: &str,
        team_id: Option<&str>,
    ) -> anyhow::Result<AgentTraceMailboxSummary> {
        let row = if let Some(team_id) = team_id {
            sqlx::query(
                r#"
                SELECT COUNT(*) AS pending_count, MAX(id) AS latest_id
                FROM team_actor_messages
                WHERE to_actor_id = ?1
                  AND status = 'pending'
                  AND run_id IN (
                      SELECT id
                      FROM team_runs
                      WHERE team_id = ?2
                        AND (
                            status IN ('submitted', 'working', 'input_required')
                            OR trim(COALESCE(json_extract(input_json, '$.bootstrap_kind'), '')) = 'shared_thread_mailbox'
                        )
                  )
                "#,
            )
            .bind(agent_id)
            .bind(team_id)
            .fetch_one(db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT COUNT(*) AS pending_count, MAX(id) AS latest_id
                FROM team_actor_messages
                WHERE to_actor_id = ?1 AND status = 'pending'
                "#,
            )
            .bind(agent_id)
            .fetch_one(db)
            .await?
        };
        Ok(AgentTraceMailboxSummary {
            pending_to_actor_count: row.get("pending_count"),
            latest_pending_message_id: row.try_get::<Option<i64>, _>("latest_id").ok().flatten(),
        })
    }

    fn event_from_row(
        event_id: i64,
        session_id: String,
        seq: String,
        ts: i64,
        stream: String,
        message: &str,
    ) -> AgentTraceEvent {
        let parsed = serde_json::from_str::<Value>(message).ok();
        let event_type = parsed
            .as_ref()
            .and_then(|value| value.get("type"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let status = parsed
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(value_to_short_string);
        let tool_call_id = parsed
            .as_ref()
            .and_then(|value| {
                value.get("tool_call_id").or_else(|| {
                    matches!(
                        event_type.as_deref(),
                        Some("tool_call" | "tool_call_update")
                    )
                    .then(|| value.get("id"))
                    .flatten()
                })
            })
            .and_then(Value::as_str)
            .map(str::to_string);
        let permission_id = parsed
            .as_ref()
            .and_then(|value| value.get("permission_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let redacted_fields = parsed
            .as_ref()
            .and_then(Value::as_object)
            .map(|object| {
                ["text", "content", "raw_input", "raw_output", "tool_call"]
                    .into_iter()
                    .filter(|field| object.contains_key(*field))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        AgentTraceEvent {
            event_id,
            session_id,
            seq,
            ts,
            stream,
            event_type,
            status,
            tool_call_id,
            permission_id,
            redacted_fields,
        }
    }

    fn value_to_short_string(value: &Value) -> Option<String> {
        match value {
            Value::String(raw) => Some(raw.clone()),
            Value::Object(object) => object
                .get("status")
                .or_else(|| object.get("state"))
                .and_then(Value::as_str)
                .map(str::to_string),
            _ => None,
        }
    }

    fn runtime_summary(
        agent: &Option<AgentTraceAgent>,
        session: &Option<AgentTraceSession>,
    ) -> AgentTraceRuntimeSummary {
        let ownership = agent
            .as_ref()
            .and_then(|agent| agent.target_node_id.as_deref())
            .map(|node_id| format!("remote:{node_id}"))
            .unwrap_or_else(|| "local".to_string());
        AgentTraceRuntimeSummary {
            ownership,
            active_session_id: session.as_ref().map(|session| session.id.clone()),
            live_state_source: "sqlite_snapshot".to_string(),
        }
    }

    fn classify_stall(
        agent: &Option<AgentTraceAgent>,
        team: &Option<AgentTraceTeam>,
        session: &Option<AgentTraceSession>,
        events: &AgentTraceEventSummary,
        permissions: &AgentTracePermissionSummary,
        mailbox: &AgentTraceMailboxSummary,
    ) -> AgentTraceVerdict {
        if agent.is_none() {
            return AgentTraceVerdict {
                layer: AgentTraceStallLayer::TargetNotFound,
                reason: "agent row is missing".to_string(),
            };
        }
        if team.as_ref().is_some_and(|team| !team.member_found) {
            return AgentTraceVerdict {
                layer: AgentTraceStallLayer::TeamMemberNotFound,
                reason: "team spec does not include the requested member".to_string(),
            };
        }
        let Some(session) = session else {
            return AgentTraceVerdict {
                layer: AgentTraceStallLayer::RuntimeNotRunning,
                reason: "no agent session row was found".to_string(),
            };
        };
        if !matches!(session.status.as_str(), "running" | "waiting_permission") {
            return AgentTraceVerdict {
                layer: AgentTraceStallLayer::RuntimeNotRunning,
                reason: format!("latest session status is {}", session.status),
            };
        }
        if session.status == "waiting_permission" || permissions.pending_count > 0 {
            return AgentTraceVerdict {
                layer: AgentTraceStallLayer::WaitingPermission,
                reason: format!(
                    "session status is {} and {} pending permission request(s) exist",
                    session.status, permissions.pending_count
                ),
            };
        }
        if mailbox.pending_to_actor_count > 0 {
            return AgentTraceVerdict {
                layer: AgentTraceStallLayer::MailboxPending,
                reason: format!(
                    "{} pending mailbox message(s) are addressed to the agent",
                    mailbox.pending_to_actor_count
                ),
            };
        }
        if events.count == 0 {
            return AgentTraceVerdict {
                layer: AgentTraceStallLayer::NoPersistedEvents,
                reason: "session is active but no persisted ACP/output events were found"
                    .to_string(),
            };
        }
        AgentTraceVerdict {
            layer: AgentTraceStallLayer::EventStreamPresent,
            reason: "persisted events exist; inspect SSE/client delivery next".to_string(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use sqlx::sqlite::SqlitePoolOptions;

        async fn test_pool() -> SqlitePool {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .expect("connect sqlite");
            sqlx::query(
                r#"
                CREATE TABLE agents (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    status TEXT NOT NULL,
                    target_node_id TEXT,
                    updated_at INTEGER NOT NULL
                );
                CREATE TABLE agent_sessions (
                    id TEXT PRIMARY KEY,
                    agent_id TEXT NOT NULL,
                    status TEXT NOT NULL,
                    started_at INTEGER NOT NULL,
                    ended_at INTEGER
                );
                CREATE TABLE team_definitions (
                    id TEXT PRIMARY KEY,
                    spec_json TEXT NOT NULL
                );
                CREATE TABLE team_runs (
                    id TEXT PRIMARY KEY,
                    team_id TEXT NOT NULL,
                    status TEXT NOT NULL,
                    input_json TEXT,
                    created_at INTEGER NOT NULL
                );
                CREATE TABLE team_actor_messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    run_id TEXT NOT NULL,
                    to_actor_id TEXT NOT NULL,
                    status TEXT NOT NULL
                );
                CREATE TABLE acp_permission_requests (
                    id TEXT PRIMARY KEY,
                    agent_id TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    tool_call_id TEXT,
                    status TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                "#,
            )
            .execute(&pool)
            .await
            .expect("create schema");
            pool
        }

        fn test_event_dir() -> PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "agenthub-agent-trace-test-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&dir).expect("create event dir");
            dir
        }

        async fn insert_agent_fixture(pool: &SqlitePool) {
            sqlx::query(
                "INSERT INTO agents (id, name, status, target_node_id, updated_at) VALUES (?1, ?2, ?3, NULL, ?4)",
            )
            .bind("worker")
            .bind("Worker")
            .bind("running")
            .bind(10_i64)
            .execute(pool)
            .await
            .expect("insert agent");
            sqlx::query(
                "INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at) VALUES (?1, ?2, ?3, ?4, NULL)",
            )
            .bind("session-1")
            .bind("worker")
            .bind("running")
            .bind(11_i64)
            .execute(pool)
            .await
            .expect("insert session");
        }

        async fn insert_event(event_dir: &Path, message: impl Into<Vec<u8>>) {
            let event_db = event_dir.join("worker.db");
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    SqliteConnectOptions::new()
                        .filename(&event_db)
                        .create_if_missing(true),
                )
                .await
                .expect("connect event db");
            sqlx::query(
                r#"
                CREATE TABLE agent_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    seq TEXT NOT NULL,
                    ts INTEGER NOT NULL,
                    stream TEXT NOT NULL,
                    message BLOB NOT NULL
                )
                "#,
            )
            .execute(&pool)
            .await
            .expect("create event schema");
            sqlx::query(
                "INSERT INTO agent_events (session_id, seq, ts, stream, message) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind("session-1")
            .bind("seq-1")
            .bind(12_i64)
            .bind("acp")
            .bind(message.into())
            .execute(&pool)
            .await
            .expect("insert event");
            pool.close().await;
        }

        #[tokio::test]
        async fn agent_trace_reports_pending_permission_without_tool_body() {
            let pool = test_pool().await;
            insert_agent_fixture(&pool).await;
            sqlx::query(
                "UPDATE agent_sessions SET status = 'waiting_permission' WHERE id = 'session-1'",
            )
            .execute(&pool)
            .await
            .expect("update session");
            sqlx::query(
                "INSERT INTO acp_permission_requests (id, agent_id, session_id, tool_call_id, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind("perm-1")
            .bind("worker")
            .bind("session-1")
            .bind("tool-1")
            .bind("pending")
            .bind(13_i64)
            .execute(&pool)
            .await
            .expect("insert permission");
            let event_dir = test_event_dir();
            insert_event(
                &event_dir,
                r#"{"type":"tool_call","id":"tool-1","status":"running","raw_input":"secret prompt","raw_output":"secret output","content":[{"text":"secret"}]}"#
                    .as_bytes()
                    .to_vec(),
            )
            .await;

            let report = collect_from_pool(
                &pool,
                event_dir.clone(),
                AgentTraceRequest {
                    agent_id: Some("worker".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("collect trace");

            assert_eq!(
                report.verdict.layer,
                AgentTraceStallLayer::WaitingPermission
            );
            assert_eq!(report.runtime.ownership, "local");
            assert_eq!(report.runtime.live_state_source, "sqlite_snapshot");
            assert_eq!(report.provider_adapter.status, "unavailable_in_db_snapshot");
            assert_eq!(report.sse.status, "unavailable_in_db_snapshot");
            assert_eq!(report.permissions.pending_permission_ids, vec!["perm-1"]);
            let latest = report.events.latest.as_ref().expect("latest event");
            assert_eq!(latest.tool_call_id.as_deref(), Some("tool-1"));
            assert!(latest.redacted_fields.contains(&"raw_input".to_string()));
            let json = serde_json::to_string(&report).expect("serialize report");
            assert!(!json.contains("secret prompt"));
            assert!(!json.contains("secret output"));
            let _ = std::fs::remove_dir_all(event_dir);
        }

        #[tokio::test]
        async fn agent_trace_decodes_compressed_acp_event_metadata() {
            let pool = test_pool().await;
            insert_agent_fixture(&pool).await;
            let event_dir = test_event_dir();
            let message = format!(
                r#"{{"type":"tool_call_update","tool_call_id":"tool-large","status":"running","raw_input":"{}"}}"#,
                "x".repeat(4096)
            );
            let encoded = crate::agent::event_message_codec::encode_message_for_storage(
                &crate::agent::OutputStream::Acp,
                &message,
            );
            insert_event(&event_dir, encoded).await;

            let report = collect_from_pool(
                &pool,
                event_dir.clone(),
                AgentTraceRequest {
                    agent_id: Some("worker".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("collect trace");

            let latest = report.events.latest.as_ref().expect("latest event");
            assert_eq!(latest.event_type.as_deref(), Some("tool_call_update"));
            assert_eq!(latest.tool_call_id.as_deref(), Some("tool-large"));
            assert!(latest.redacted_fields.contains(&"raw_input".to_string()));
            let _ = std::fs::remove_dir_all(event_dir);
        }

        #[tokio::test]
        async fn agent_trace_resolves_team_member_to_agent_id() {
            let pool = test_pool().await;
            insert_agent_fixture(&pool).await;
            sqlx::query("INSERT INTO team_definitions (id, spec_json) VALUES (?1, ?2)")
                .bind("team-1")
                .bind(r#"{"members":[{"member_id":"worker","role":"worker"}]}"#)
                .execute(&pool)
                .await
                .expect("insert team");
            sqlx::query(
                "INSERT INTO team_runs (id, team_id, status, created_at) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind("run-1")
            .bind("team-1")
            .bind("working")
            .bind(20_i64)
            .execute(&pool)
            .await
            .expect("insert run");
            sqlx::query(
                "INSERT INTO team_actor_messages (run_id, to_actor_id, status) VALUES (?1, ?2, ?3)",
            )
            .bind("run-1")
            .bind("worker")
            .bind("pending")
            .execute(&pool)
            .await
            .expect("insert mailbox");

            let event_dir = test_event_dir();
            let report = collect_from_pool(
                &pool,
                event_dir.clone(),
                AgentTraceRequest {
                    team_id: Some("team-1".to_string()),
                    member_id: Some("worker".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("collect trace");

            assert_eq!(report.target.agent_id, "worker");
            assert_eq!(
                report.team.as_ref().expect("team").latest_run_id.as_deref(),
                Some("run-1")
            );
            assert_eq!(report.verdict.layer, AgentTraceStallLayer::MailboxPending);
            let _ = std::fs::remove_dir_all(event_dir);
        }

        #[tokio::test]
        async fn agent_trace_ignores_pending_mailbox_for_terminal_team_runs() {
            let pool = test_pool().await;
            insert_agent_fixture(&pool).await;
            sqlx::query("INSERT INTO team_definitions (id, spec_json) VALUES (?1, ?2)")
                .bind("team-1")
                .bind(r#"{"members":[{"member_id":"worker","role":"worker"}]}"#)
                .execute(&pool)
                .await
                .expect("insert team");
            sqlx::query(
                "INSERT INTO team_runs (id, team_id, status, created_at) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind("run-canceled")
            .bind("team-1")
            .bind("canceled")
            .bind(20_i64)
            .execute(&pool)
            .await
            .expect("insert run");
            sqlx::query(
                "INSERT INTO team_actor_messages (run_id, to_actor_id, status) VALUES (?1, ?2, ?3)",
            )
            .bind("run-canceled")
            .bind("worker")
            .bind("pending")
            .execute(&pool)
            .await
            .expect("insert mailbox");

            let event_dir = test_event_dir();
            let report = collect_from_pool(
                &pool,
                event_dir.clone(),
                AgentTraceRequest {
                    team_id: Some("team-1".to_string()),
                    member_id: Some("worker".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("collect trace");

            assert_eq!(report.mailbox.pending_to_actor_count, 0);
            assert_ne!(report.verdict.layer, AgentTraceStallLayer::MailboxPending);
            let _ = std::fs::remove_dir_all(event_dir);
        }

        #[tokio::test]
        async fn agent_trace_includes_shared_thread_mailbox_run_pending_messages() {
            let pool = test_pool().await;
            insert_agent_fixture(&pool).await;
            sqlx::query("INSERT INTO team_definitions (id, spec_json) VALUES (?1, ?2)")
                .bind("team-1")
                .bind(r#"{"members":[{"member_id":"worker","role":"worker"}]}"#)
                .execute(&pool)
                .await
                .expect("insert team");
            sqlx::query(
                "INSERT INTO team_runs (id, team_id, status, input_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind("shared-run")
            .bind("team-1")
            .bind("completed")
            .bind(r#"{"bootstrap_kind":"shared_thread_mailbox"}"#)
            .bind(20_i64)
            .execute(&pool)
            .await
            .expect("insert run");
            sqlx::query(
                "INSERT INTO team_actor_messages (run_id, to_actor_id, status) VALUES (?1, ?2, ?3)",
            )
            .bind("shared-run")
            .bind("worker")
            .bind("pending")
            .execute(&pool)
            .await
            .expect("insert mailbox");

            let event_dir = test_event_dir();
            let report = collect_from_pool(
                &pool,
                event_dir.clone(),
                AgentTraceRequest {
                    team_id: Some("team-1".to_string()),
                    member_id: Some("worker".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("collect trace");

            assert_eq!(report.mailbox.pending_to_actor_count, 1);
            assert_eq!(report.verdict.layer, AgentTraceStallLayer::MailboxPending);
            let _ = std::fs::remove_dir_all(event_dir);
        }

        #[tokio::test]
        async fn agent_trace_reports_missing_standalone_target() {
            let pool = test_pool().await;
            let event_dir = test_event_dir();

            let report = collect_from_pool(
                &pool,
                event_dir.clone(),
                AgentTraceRequest {
                    agent_id: Some("missing-agent".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("collect trace");

            assert_eq!(report.target.agent_id, "missing-agent");
            assert!(report.agent.is_none());
            assert_eq!(report.verdict.layer, AgentTraceStallLayer::TargetNotFound);
            assert_eq!(report.runtime.ownership, "local");
            assert!(render_human(&report).contains("agent: <missing>"));
            let _ = std::fs::remove_dir_all(event_dir);
        }

        #[tokio::test]
        async fn agent_trace_reports_missing_team_member_and_applies_live_overlay() {
            let pool = test_pool().await;
            insert_agent_fixture(&pool).await;
            sqlx::query("INSERT INTO team_definitions (id, spec_json) VALUES (?1, ?2)")
                .bind("team-1")
                .bind(r#"{"members":[{"member_id":"other","role":"worker"}]}"#)
                .execute(&pool)
                .await
                .expect("insert team");
            let event_dir = test_event_dir();

            let mut report = collect_from_pool(
                &pool,
                event_dir.clone(),
                AgentTraceRequest {
                    team_id: Some("team-1".to_string()),
                    member_id: Some("worker".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("collect trace");

            let team = report.team.as_ref().expect("team summary");
            assert!(!team.member_found);
            assert_eq!(
                report.verdict.layer,
                AgentTraceStallLayer::TeamMemberNotFound
            );

            apply_live_overlay(
                &mut report,
                AgentTraceLiveOverlay {
                    runtime: AgentTraceRuntimeSummary {
                        ownership: "local".to_string(),
                        active_session_id: Some("session-1".to_string()),
                        live_state_source: "live_backend".to_string(),
                    },
                    provider_adapter: AgentTraceAvailability {
                        status: "prompt_active".to_string(),
                        note: "redacted provider snapshot".to_string(),
                        details: serde_json::json!({"active_prompt_count": 1}),
                    },
                    sse: AgentTraceAvailability {
                        status: "subscribers_active".to_string(),
                        note: "redacted sse snapshot".to_string(),
                        details: serde_json::json!({"output_subscriber_count": 2}),
                    },
                },
            );
            assert_eq!(report.runtime.live_state_source, "live_backend");
            assert_eq!(report.provider_adapter.status, "prompt_active");
            assert_eq!(report.sse.status, "subscribers_active");
            let rendered = render_human(&report);
            assert!(rendered.contains("team.member_found: false"));
            assert!(rendered.contains("provider_adapter.status: prompt_active"));
            apply_live_overlay(
                &mut report,
                AgentTraceLiveOverlay {
                    runtime: AgentTraceRuntimeSummary {
                        ownership: "local".to_string(),
                        active_session_id: Some("session-1".to_string()),
                        live_state_source: "live_backend".to_string(),
                    },
                    provider_adapter: AgentTraceAvailability {
                        status: "prompt_stale".to_string(),
                        note: "redacted provider snapshot".to_string(),
                        details: serde_json::json!({
                            "active_prompt_count": 1,
                            "pending_permission_count": 0,
                        }),
                    },
                    sse: AgentTraceAvailability {
                        status: "subscribers_active".to_string(),
                        note: "redacted sse snapshot".to_string(),
                        details: serde_json::json!({"output_subscriber_count": 2}),
                    },
                },
            );
            assert_eq!(
                report.verdict.layer,
                AgentTraceStallLayer::ProviderPromptStale
            );
            let _ = std::fs::remove_dir_all(event_dir);
        }

        #[test]
        fn agent_trace_request_validates_target_shape() {
            assert!(
                AgentTraceRequest {
                    agent_id: Some("agent".to_string()),
                    ..Default::default()
                }
                .validate()
                .is_ok()
            );
            assert!(
                AgentTraceRequest {
                    agent_id: Some("  ".to_string()),
                    team_id: Some("team".to_string()),
                    member_id: Some("member".to_string()),
                    ..Default::default()
                }
                .validate()
                .is_ok()
            );
            assert!(
                AgentTraceRequest {
                    team_id: Some("team".to_string()),
                    member_id: Some("member".to_string()),
                    ..Default::default()
                }
                .validate()
                .is_ok()
            );
            assert!(AgentTraceRequest::default().validate().is_err());
        }
    }
}
