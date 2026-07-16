use agenthub_auth_domain::UserCapability;
use axum::http::HeaderMap;

use crate::api::error::ApiError;
use crate::auth::UserRecord;
use crate::state::AppState;

pub async fn require_user(headers: &HeaderMap, state: &AppState) -> Result<UserRecord, ApiError> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("missing authorization token"))?;

    let user = state
        .auth
        .validate_session(token)
        .await
        .map_err(|_| ApiError::unauthorized("invalid token"))?;
    Ok(user)
}

pub async fn require_root(headers: &HeaderMap, state: &AppState) -> Result<UserRecord, ApiError> {
    let user = require_user(headers, state).await?;
    if !user.has_capability(UserCapability::InstanceConfigure) {
        return Err(ApiError::unauthorized("root required"));
    }
    Ok(user)
}

pub async fn require_capability(
    headers: &HeaderMap,
    state: &AppState,
    capability: UserCapability,
) -> Result<UserRecord, ApiError> {
    let user = require_user(headers, state).await?;
    if !user.has_capability(capability) {
        return Err(ApiError::unauthorized(&format!(
            "{} required",
            capability.as_str()
        )));
    }
    Ok(user)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    const FORBIDDEN_HUMAN_ROLE_CHECKS: [&str; 8] = [
        "user.role == \"root\"",
        "user.role != \"root\"",
        "user.role == \"admin\"",
        "user.role != \"admin\"",
        "user.role == \"operator\"",
        "user.role != \"operator\"",
        "user.role == \"viewer\"",
        "user.role != \"viewer\"",
    ];

    #[test]
    fn api_code_does_not_bypass_capability_authz_for_human_roles() {
        let api_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api");
        let mut violations = Vec::new();
        collect_human_role_authz_violations(&api_dir, &mut violations);

        assert!(
            violations.is_empty(),
            "direct human role checks must go through api::authz capability helpers:\n{}",
            violations.join("\n")
        );
    }

    fn collect_human_role_authz_violations(dir: &Path, violations: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("read api dir") {
            let entry = entry.expect("read api dir entry");
            let path = entry.path();
            if path.is_dir() {
                collect_human_role_authz_violations(&path, violations);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some("authz.rs") {
                continue;
            }

            let content = fs::read_to_string(&path).expect("read api source");
            for (line_index, line) in content.lines().enumerate() {
                if FORBIDDEN_HUMAN_ROLE_CHECKS
                    .iter()
                    .any(|forbidden| line.contains(forbidden))
                {
                    violations.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        line_index + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
}
