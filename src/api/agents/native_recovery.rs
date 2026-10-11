use super::*;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecoveryQuery {
    pub local_session_id: String,
}

pub(super) async fn query(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Query(query): Query<RecoveryQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    let recovery = state
        .agents
        .query_native_recovery(&agent_id, &query.local_session_id)
        .await
        .map_err(|_| ApiError::conflict("Native recovery state is unavailable for this session"))?;
    Ok(Json(serde_json::to_value(recovery)?))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReconcileRequest {
    pub local_session_id: String,
    pub target: agenthub_rara::RecoveryTarget,
    pub note: String,
}

pub(super) async fn reconcile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
    Json(payload): Json<ReconcileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _user = require_capability(&headers, &state, UserCapability::RuntimeOperate).await?;
    payload
        .target
        .validate()
        .map_err(|_| ApiError::bad_request("Invalid native recovery target"))?;
    agenthub_rara::RecoveryResolution { recovery_id: payload.target.recovery_id.clone(), note: payload.note.clone() }
        .validate().map_err(|_| ApiError::bad_request("A nonempty reconciliation note of at most 4096 bytes without control characters is required"))?;
    state.agents.reconcile_native_recovery(&agent_id, &payload.local_session_id, payload.target, payload.note)
        .await.map_err(|_| ApiError::conflict("Native recovery could not be confirmed; refresh the current session state before retrying"))?;
    Ok(ok_response())
}
