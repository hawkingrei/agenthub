use agenthub_agent_domain::app_events::{APP_EVENT_MAX_BYTES, AppEventNotification};
use agenthub_db::app_registry::{AppEventIntakeError, AppEventReceipt, AppRegistry};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State, rejection::BytesRejection},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;

use super::{
    ApiError,
    event_signature::{AppEventSignature, decode_fixed},
};
use crate::state::AppState;

/// This route authenticates App signatures, not browser bearer tokens or provider credentials.
pub(super) async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
    body: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<AppEventReceipt>), ApiError> {
    let body = body.map_err(|_| ApiError::bad_request("invalid bounded app event"))?;
    if body.len() > APP_EVENT_MAX_BYTES {
        return Err(ApiError::bad_request("invalid bounded app event"));
    }
    let now = Utc::now().timestamp();
    let authentication = || ApiError::unauthorized("app event authentication failed");
    let signature = AppEventSignature::parse(&headers, now).map_err(|_| authentication())?;
    let registry = AppRegistry::new(state.db);
    let key = registry
        .event_signing_key(&app_id)
        .await
        .map_err(|_| ApiError::from(anyhow::anyhow!("app event key lookup failed")))?
        .filter(|key| key.config.version == signature.key_version)
        .ok_or_else(authentication)?;
    let material = std::env::var(&key.credential_env).map_err(|_| authentication())?;
    let secret = decode_fixed(&material).map_err(|_| authentication())?;
    signature
        .verify(&app_id, &body, &secret)
        .map_err(|_| authentication())?;
    let event: AppEventNotification = serde_json::from_slice(&body)
        .map_err(|_| ApiError::bad_request("invalid app event notification"))?;
    event
        .validate()
        .map_err(|_| ApiError::bad_request("invalid app event notification"))?;
    let receipt = registry
        .accept_signed_event(&app_id, signature.key_version, &event, now)
        .await
        .map_err(|error| match error.downcast_ref::<AppEventIntakeError>() {
            Some(AppEventIntakeError::SigningAuthority) => authentication(),
            Some(AppEventIntakeError::Unauthorized) => {
                ApiError::forbidden("app event route is not authorized")
            }
            Some(AppEventIntakeError::IdConflict | AppEventIntakeError::CursorReplay) => {
                ApiError::conflict("app event identity or cursor conflicts")
            }
            Some(AppEventIntakeError::Capacity) => {
                ApiError::too_many_requests("app event intake capacity reached")
            }
            Some(AppEventIntakeError::Disabled) => {
                ApiError::conflict("app event target execution is disabled")
            }
            None => ApiError::from(anyhow::anyhow!("app event intake failed")),
        })?;
    Ok((
        if receipt.duplicate {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        Json(receipt),
    ))
}
