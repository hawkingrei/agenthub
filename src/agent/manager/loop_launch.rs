use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use agenthub_agent_domain::loop_runtime::{LoopLaunchSnapshot, LoopReservation, LoopSessionPolicy};
use agenthub_db::loop_runtime::LoopStore;
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::acp::{AcpActorSkillContext, AcpLoopLaunchConfig, LOOP_ACTIVATION_CONTRACT_VERSION};
use crate::internal::auth::{InternalAction, InternalAuthz, InternalRole, LoopExecutionClaims};
use crate::internal::p2p::NodeCredentialRequest;
use crate::loop_credentials::{LoopCredentialEnvelope, LoopCredentialFile};
use crate::team::TeamManager;

use super::acp_provider::{AcpProviderSpec, codex_reasoning_effort_for_thinking_level};
use super::{AgentInput, AgentManager};

const LOOP_ENTRY_PROMPT: &str = "Run one bounded AgentHub activation. Recover current role and authority with `agenthub actor team-members --json`, canonical work with `agenthub actor team-tasks --json`, and the addressed mailbox with `agenthub actor inbox --json`. The mailbox run is stable transport identity; this activation does not create a task attempt. Respect canonical assignment and task acceptance authority. Provider reasoning and native tool rounds belong to this activation. Record durable task evidence before reporting progress. End with `agenthub actor loop-finish --outcome-file <path> --json`; `agenthub actor help loop-finish` describes the output contract. A provider exit or completed prompt is not an outcome. Do not poll for future work or start another resident loop.";

#[derive(Clone)]
pub(crate) struct LoopControlEndpoint {
    pub target: String,
    pub authz: InternalAuthz,
    pub ca_cert_path: Option<String>,
}

pub(super) struct LoopCredentialState {
    pub file: Arc<LoopCredentialFile>,
    pub role: InternalRole,
    pub run_id: String,
}

impl AgentManager {
    pub(crate) async fn publish_loop_control_endpoint(&self, endpoint: LoopControlEndpoint) {
        *self.loop_control_endpoint.write().await = Some(endpoint);
    }

    pub(super) async fn refresh_loop_credentials(
        &self,
        reservation: &LoopReservation,
    ) -> anyhow::Result<()> {
        let credentials = self.loop_credentials.lock().await;
        let Some(credentials) = credentials.get(&reservation.actor_id) else {
            return Ok(());
        };
        let endpoint = self
            .loop_control_endpoint
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("local loop control endpoint is unavailable"))?;
        let activation_id = reservation
            .activation_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("loop activation identity is required"))?;
        let issued = endpoint.authz.issue_loop_access_token(
            NodeCredentialRequest {
                source_node_id: crate::agent::AGENT_NODE_MAIN_ID.into(),
                role: credentials.role.as_str().into(),
                actor_id: Some(reservation.actor_id.clone()),
                run_id: Some(credentials.run_id.clone()),
                permissions: [
                    InternalAction::MessageSend,
                    InternalAction::InboxList,
                    InternalAction::MessageAck,
                    InternalAction::TeamRead,
                    InternalAction::TeamTaskWrite,
                    InternalAction::PermissionReview,
                    InternalAction::LoopFinish,
                ]
                .into_iter()
                .map(|action| action.as_str().to_string())
                .collect(),
                scope: Vec::new(),
                audience: Vec::new(),
                ttl_seconds: i64::from(reservation.lease_seconds).max(60),
            },
            LoopExecutionClaims {
                activation_id: activation_id.clone(),
                generation: reservation.generation,
            },
        )?;
        credentials.file.replace(&LoopCredentialEnvelope {
            actor_id: reservation.actor_id.clone(),
            run_id: credentials.run_id.clone(),
            activation_id: activation_id.clone(),
            generation: reservation.generation,
            target: endpoint.target,
            access_token: issued.access_token,
            expires_at: issued.expires_at,
            ca_cert_path: endpoint.ca_cert_path,
        })
    }

    pub(super) async fn resolve_loop_launch(
        &self,
        agent: &crate::agent::AgentRecord,
        provider: AcpProviderSpec,
        context: &AcpActorSkillContext,
        workdir: &str,
        command: &str,
        args: &[String],
    ) -> anyhow::Result<(
        AcpLoopLaunchConfig,
        LoopSessionPolicy,
        Vec<(String, String)>,
    )> {
        let reservation = self
            .loop_reservations
            .lock()
            .await
            .get(&agent.id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("loop execution reservation is required"))?;
        let store = LoopStore::new(self.db.clone());
        let policy = store
            .policy(&reservation.team_id, &agent.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("loop policy is missing"))?;
        let mut launch = AcpLoopLaunchConfig::resolve(
            Path::new(workdir),
            policy.session_policy == LoopSessionPolicy::Resume,
        );
        if provider.uses_default_mode_config() {
            launch.mode_id = super::session::effective_acp_default_mode(
                provider,
                agent.codex_acp_default_mode.as_deref(),
                self.acp_default_mode.as_deref(),
                true,
            )
            .map(str::to_owned);
        }
        if provider.applies_runtime_profile_via_session_config() {
            launch.model_id = agent.runtime_model.clone();
            if let Some(level) = &agent.thinking_level {
                let effort = codex_reasoning_effort_for_thinking_level(level)
                    .ok_or_else(|| anyhow::anyhow!("unsupported loop thinking level"))?;
                launch
                    .config
                    .push(("reasoning_effort".into(), effort.into()));
            }
        }
        let mut digest = Sha256::new();
        digest.update(serde_json::to_vec(&(
            command,
            args,
            context,
            &agent.runtime_model,
            &agent.thinking_level,
        ))?);
        digest.update(launch.fingerprint_material()?);
        let snapshot = LoopLaunchSnapshot {
            version: 1,
            provider_id: provider.id.into(),
            configuration_digest: digest
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            entry_prompt_version: LOOP_ACTIVATION_CONTRACT_VERSION.into(),
            session_policy: policy.session_policy,
            workspace: workdir.into(),
            model: agent.runtime_model.clone(),
            thinking_level: agent.thinking_level.clone(),
        };
        store
            .record_launch(&reservation, &snapshot, Utc::now().timestamp())
            .await?;
        let role = InternalRole::parse(context.member_role.as_deref().unwrap_or_default())
            .filter(|role| matches!(role, InternalRole::Coordinator | InternalRole::Worker))
            .ok_or_else(|| anyhow::anyhow!("loop activation requires a supported member role"))?;
        let run_id = context
            .current_run_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("loop mailbox identity is required"))?;
        let file = Arc::new(LoopCredentialFile::create()?);
        let path = file.path.to_string_lossy().to_string();
        self.loop_credentials
            .lock()
            .await
            .insert(agent.id.clone(), LoopCredentialState { file, role, run_id });
        self.refresh_loop_credentials(&reservation).await?;
        Ok((
            launch,
            policy.session_policy,
            vec![
                (
                    crate::loop_credentials::LOOP_CREDENTIAL_FILE_ENV.into(),
                    path,
                ),
                (
                    crate::loop_credentials::LOOP_ACTIVATION_ENV.into(),
                    reservation.activation_id.unwrap(),
                ),
            ],
        ))
    }

    pub(crate) fn spawn_loop_worker(&self, teams: Arc<TeamManager>) -> anyhow::Result<()> {
        if !cfg!(target_os = "linux") {
            return Ok(());
        }
        let manager = self.clone();
        let cancel = self.daemon_tasks.background_cancellation();
        self.daemon_tasks.spawn_background_worker("local-agent-loop", async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let store = LoopStore::new(manager.db.clone());
            loop {
                tokio::select! { _ = cancel.cancelled() => return Ok(()), _ = interval.tick() => {} }
                if manager.loop_control_endpoint.read().await.is_none() { continue; }
                let reservation = match store.admit_next(&manager.loop_owner_id, Utc::now().timestamp()).await {
                    Ok(Some(reservation)) => reservation,
                    Ok(None) => continue,
                    Err(error) => { tracing::warn!(%error, "loop admission scan failed"); continue; }
                };
                manager.track_loop_reservation(reservation.clone()).await?;
                let task_manager = manager.clone();
                let task_teams = teams.clone();
                let task_reservation = reservation.clone();
                let started = manager.daemon_tasks.spawn_runtime_task(format!("loop-activation:{}:{}", reservation.actor_id, reservation.generation), async move {
                    if let Err(error) = task_manager.execute_loop_activation(task_teams, task_reservation.clone()).await {
                        tracing::warn!(actor_id = task_reservation.actor_id, generation = task_reservation.generation, %error, "local loop activation failed");
                        task_manager.fence_loop_reservation(&task_reservation).await?;
                    }
                    Ok(())
                });
                if started.is_err() { manager.fence_loop_reservation(&reservation).await?; }
            }
        })
    }

    async fn execute_loop_activation(
        &self,
        teams: Arc<TeamManager>,
        reservation: LoopReservation,
    ) -> anyhow::Result<()> {
        let team = teams.get_team(&reservation.team_id).await?;
        let role = team
            .spec
            .get("members")
            .and_then(serde_json::Value::as_array)
            .and_then(|members| {
                members.iter().find(|member| {
                    member.get("member_id").and_then(serde_json::Value::as_str)
                        == Some(&reservation.actor_id)
                })
            })
            .and_then(|member| member.get("role"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("loop member role is missing"))?;
        let mailbox = teams
            .ensure_loop_mailbox_partition(&reservation.team_id)
            .await?;
        let store = LoopStore::new(self.db.clone());
        store
            .bind_mailbox(&reservation, &mailbox.id, Utc::now().timestamp())
            .await?;
        let mut context = crate::team::build_team_member_actor_context_for_role(
            &reservation.team_id,
            Some(&mailbox.id),
            &reservation.actor_id,
            role,
        );
        context.contract_version = Some(LOOP_ACTIVATION_CONTRACT_VERSION.into());
        let session_id = self.start_loop_agent(&reservation, context).await?;
        let reservation = store
            .reservation(&reservation.team_id, &reservation.actor_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("loop reservation disappeared during startup"))?;
        store
            .mark_running(&reservation, Utc::now().timestamp())
            .await?;
        let submission = format!(
            "loop-entry:{}:{}",
            reservation.activation_id.as_deref().unwrap_or_default(),
            reservation.generation
        );
        self.send_input_inner(
            &reservation.actor_id,
            LOOP_ENTRY_PROMPT,
            &[],
            Some(&submission),
            Some(&session_id),
            None,
        )
        .await?;
        let cancellation = self.daemon_tasks.runtime_cancellation();
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => return self.fence_loop_reservation(&reservation).await,
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
            let diagnostics = {
                let handles = self.inner.read().await;
                let Some(handle) = handles
                    .get(&reservation.actor_id)
                    .filter(|handle| handle.session_id == session_id)
                else {
                    return Ok(());
                };
                let AgentInput::Acp(acp) = &handle.input else {
                    anyhow::bail!("loop provider is not ACP");
                };
                acp.diagnostics()
            };
            if diagnostics.command_channel_closed
                || (diagnostics.last_submission_id.as_deref() == Some(&submission)
                    && diagnostics.active_submission_ids.is_empty()
                    && diagnostics.active_prompt_count == 0)
            {
                // A transport turn may end without a durable outcome. Cleanup classifies that as interrupted.
                return self.fence_loop_reservation(&reservation).await;
            }
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
