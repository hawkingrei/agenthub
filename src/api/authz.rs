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

    const API_SOURCES: [(&str, &str); 13] = [
        ("admin.rs", include_str!("admin.rs")),
        ("agent_nodes.rs", include_str!("agent_nodes.rs")),
        ("agents.rs", include_str!("agents.rs")),
        ("auth.rs", include_str!("auth.rs")),
        ("diagnostics.rs", include_str!("diagnostics.rs")),
        ("error.rs", include_str!("error.rs")),
        ("join.rs", include_str!("join.rs")),
        ("linkers.rs", include_str!("linkers.rs")),
        ("mod.rs", include_str!("mod.rs")),
        ("openapi/mod.rs", include_str!("openapi/mod.rs")),
        ("push.rs", include_str!("push.rs")),
        ("settings.rs", include_str!("settings.rs")),
        ("teams.rs", include_str!("teams.rs")),
    ];

    #[test]
    fn api_code_does_not_bypass_capability_authz_for_human_roles() {
        let mut violations = Vec::new();
        for (path, content) in API_SOURCES {
            collect_human_role_authz_violations(path, content, &mut violations);
        }

        assert!(
            violations.is_empty(),
            "direct human role checks must go through api::authz capability helpers:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn api_routes_do_not_use_authentication_only_user_gate() {
        let mut violations = Vec::new();
        for (path, content) in API_SOURCES {
            collect_authentication_only_route_gate_violations(path, content, &mut violations);
        }

        assert!(
            violations.is_empty(),
            "api routes must use require_capability or documented require_root gates instead of require_user:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn api_root_only_gates_stay_on_reviewed_security_boundaries() {
        let mut callsites = Vec::new();
        for (path, content) in API_SOURCES {
            collect_root_only_gate_callsites(path, content, &mut callsites);
        }

        let mut expected = REVIEWED_ROOT_ONLY_CALLS
            .iter()
            .map(|callsite| (*callsite).to_string())
            .collect::<Vec<_>>();
        callsites.sort();
        expected.sort();

        assert_eq!(
            callsites, expected,
            "require_root callsites must be reviewed security boundaries; normal operations should use capability gates"
        );
    }

    fn collect_human_role_authz_violations(
        path: &str,
        content: &str,
        violations: &mut Vec<String>,
    ) {
        for (line_index, line) in content.lines().enumerate() {
            if FORBIDDEN_HUMAN_ROLE_CHECKS
                .iter()
                .any(|forbidden| line.contains(forbidden))
            {
                violations.push(format!("{}:{}: {}", path, line_index + 1, line.trim()));
            }
        }
    }

    fn collect_authentication_only_route_gate_violations(
        path: &str,
        content: &str,
        violations: &mut Vec<String>,
    ) {
        let normalized: String = content.chars().filter(|c| !c.is_whitespace()).collect();
        let has_authentication_only_gate = normalized.contains("require_user(&headers,&state)")
            || normalized.contains("require_user(headers,state)");
        if !has_authentication_only_gate {
            return;
        }

        for (line_index, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("require_user") {
                violations.push(format!("{}:{}: {}", path, line_index + 1, trimmed));
            }
        }
    }

    const REVIEWED_ROOT_ONLY_CALLS: [&str; 10] = [
        "admin.rs::get_settings",
        "admin.rs::join_start",
        "admin.rs::list_audits",
        "admin.rs::list_devices",
        "admin.rs::migrate_team_messages_archive",
        "admin.rs::revoke_device",
        "admin.rs::set_passkey_enabled",
        "agent_nodes.rs::get_agent_node_bootstrap",
        "push.rs::vapid_info",
        "push.rs::vapid_rotate",
    ];

    fn collect_root_only_gate_callsites(path: &str, content: &str, callsites: &mut Vec<String>) {
        let mut current_fn: Option<String> = None;
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("async fn ") {
                let name = rest
                    .split_once('(')
                    .map(|(name, _)| name)
                    .unwrap_or(rest)
                    .to_string();
                current_fn = Some(name);
            }
            if trimmed.contains("require_root(") {
                let function = current_fn
                    .as_deref()
                    .unwrap_or("<unknown-function>")
                    .to_string();
                callsites.push(format!("{path}::{function}"));
            }
        }
    }
}
