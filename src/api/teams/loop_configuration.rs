use agenthub_agent_domain::loop_runtime::{
    LoopLimits, LoopPolicy, LoopPolicyState, LoopSessionPolicy,
};
use agenthub_db::loop_runtime::{LoopPolicyUpdate, LoopStore, LoopStoreError};
use chrono::Utc;

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoopConfigurationRequest {
    expected_revision: i64,
    state: LoopPolicyState,
    session_policy: LoopSessionPolicy,
    limits: LoopLimits,
}

#[derive(Serialize)]
pub(super) struct LoopConfigurationResponse {
    policy: Option<LoopPolicy>,
    preflight: crate::agent::LoopPreflight,
}

pub(super) async fn get_loop_configuration(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id)): Path<(String, String)>,
) -> Result<Json<LoopConfigurationResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeInspect).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_member(&team, &actor_id)?;
    let policy = LoopStore::new(state.db.clone())
        .policy(&team_id, &actor_id)
        .await
        .map_err(map_team_internal_error)?;
    let session_policy = policy
        .as_ref()
        .map(|policy| policy.session_policy)
        .unwrap_or(LoopSessionPolicy::Fresh);
    let preflight = state
        .agents
        .loop_preflight(&team.id, &team.spec, &actor_id, session_policy)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(LoopConfigurationResponse { policy, preflight }))
}

pub(super) async fn set_loop_configuration(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id)): Path<(String, String)>,
    Json(request): Json<LoopConfigurationRequest>,
) -> Result<Json<LoopConfigurationResponse>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    require_member(&team, &actor_id)?;
    request
        .limits
        .validate()
        .map_err(|_| ApiError::bad_request("invalid bounded loop limits"))?;
    let manager = state.agents.clone();
    let result = manager
        .configure_actors_owned(vec![actor_id.clone()], async move {
            let team = state.teams.get_team(&team_id).await?;
            anyhow::ensure!(
                crate::team::TeamManager::uses_loop_execution(&team.spec),
                LoopStoreError::ScopeMismatch
            );
            let store = LoopStore::new(state.db.clone());
            let preflight = state
                .agents
                .loop_preflight(&team.id, &team.spec, &actor_id, request.session_policy)
                .await?;
            if request.state == LoopPolicyState::Enabled && !preflight.ready {
                anyhow::bail!("loop preflight failed: {}", preflight.blockers.join(", "));
            }
            let policy = store
                .configure(
                    LoopPolicyUpdate {
                        actor_id: &actor_id,
                        team_id: &team_id,
                        expected_revision: request.expected_revision,
                        state: request.state,
                        session_policy: request.session_policy,
                        limits: &request.limits,
                    },
                    Utc::now().timestamp(),
                )
                .await?;
            Ok(LoopConfigurationResponse {
                policy: Some(policy),
                preflight,
            })
        })
        .await
        .map_err(|error| {
            if error.to_string().starts_with("loop preflight failed:") {
                ApiError::conflict(&error.to_string())
            } else {
                map_team_internal_error(error)
            }
        })?;
    Ok(Json(result))
}

pub(super) fn require_member(team: &TeamDefinitionRecord, actor_id: &str) -> Result<(), ApiError> {
    if !parse_member_ids(team.spec.get("members"))?.contains(actor_id) {
        return Err(ApiError::not_found("team member not found"));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoopActivationRequest {
    pub source_key: String,
    pub task_id: Option<String>,
}

pub(super) async fn request_loop_activation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((team_id, actor_id)): Path<(String, String)>,
    Json(request): Json<LoopActivationRequest>,
) -> Result<Json<agenthub_agent_domain::loop_runtime::LoopTriggerReceipt>, ApiError> {
    let user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let team = load_team_for_user(&state, &team_id, &user).await?;
    require_teamspace_role(&state, &team, &user, &["owner"]).await?;
    require_member(&team, &actor_id)?;
    for id in std::iter::once(request.source_key.as_str()).chain(request.task_id.as_deref()) {
        agenthub_agent_domain::loop_runtime::validate_loop_id(id)
            .map_err(|_| ApiError::bad_request("invalid bounded loop identifier"))?;
    }
    let context = crate::team::loop_context::LoopSchedulingContext {
        user_id: Some(user.id),
        ..Default::default()
    };
    let receipt = crate::team::loop_context::with_scheduling_context(
        context,
        state.teams.request_loop_activation(
            &team_id,
            &actor_id,
            &request.source_key,
            request.task_id.as_deref(),
        ),
    )
    .await
    .map_err(map_team_internal_error)?;
    Ok(Json(receipt))
}

pub(super) async fn update_team_spec_owned(
    state: &AppState,
    current: &TeamDefinitionRecord,
    expected_updated_at: i64,
    spec: Value,
) -> anyhow::Result<Option<TeamDefinitionRecord>> {
    let mut actors = parse_member_ids(current.spec.get("members"))
        .map_err(|_| anyhow::anyhow!("invalid current members"))?;
    actors.extend(
        parse_member_ids(spec.get("members"))
            .map_err(|_| anyhow::anyhow!("invalid next members"))?,
    );
    let teams = state.teams.clone();
    let team_id = current.id.clone();
    state
        .agents
        .configure_actors_owned(actors.into_iter().collect(), async move {
            teams
                .update_team_spec_if_unchanged(&team_id, expected_updated_at, spec)
                .await
        })
        .await
}
