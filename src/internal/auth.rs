use std::collections::HashSet;

use jwt_simple::algorithms::MACLike;
use jwt_simple::prelude::{Claims, Duration, HS256Key};
use serde::{Deserialize, Serialize};
use tonic::{Status, metadata::MetadataMap};

const AUTHORIZATION_HEADER: &str = "authorization";
const TOKEN_PREFIX: &str = "Bearer ";

#[derive(Debug, Clone)]
pub struct InternalAuthzConfig {
    pub shared_secret: String,
    pub expected_issuer: Option<String>,
    pub expected_audience: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalAccessClaims {
    pub role: String,
    pub actor_id: Option<String>,
    pub run_id: Option<String>,
    pub permissions: Vec<String>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalRole {
    Leader,
    Worker,
    Orchestrator,
}

impl InternalRole {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "leader" => Some(Self::Leader),
            "worker" => Some(Self::Worker),
            "orchestrator" => Some(Self::Orchestrator),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Leader => "leader",
            Self::Worker => "worker",
            Self::Orchestrator => "orchestrator",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InternalPrincipal {
    pub role: InternalRole,
    pub actor_id: Option<String>,
    pub run_id: Option<String>,
    permissions: HashSet<String>,
}

impl InternalPrincipal {
    pub fn has_permission(&self, action: InternalAction) -> bool {
        self.permissions.contains("*") || self.permissions.contains(action.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InternalAction {
    MessageSend,
    InboxList,
    MessageAck,
    StepTransition,
    NodeIssue,
}

impl InternalAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MessageSend => "team:message:send",
            Self::InboxList => "team:inbox:list",
            Self::MessageAck => "team:message:ack",
            Self::StepTransition => "team:step:transition",
            Self::NodeIssue => "team:node:issue",
        }
    }
}

#[derive(Clone)]
pub struct InternalAuthz {
    key: HS256Key,
    expected_issuer: Option<String>,
    expected_audience: Option<String>,
}

impl InternalAuthz {
    pub fn new(config: InternalAuthzConfig) -> Self {
        Self {
            key: HS256Key::from_bytes(config.shared_secret.as_bytes()),
            expected_issuer: config.expected_issuer,
            expected_audience: config.expected_audience,
        }
    }

    pub fn authenticate(&self, metadata: &MetadataMap) -> Result<InternalPrincipal, Status> {
        let token = bearer_token(metadata)?;
        let claims = self
            .key
            .verify_token::<InternalAccessClaims>(token, None)
            .map_err(|err| Status::unauthenticated(format!("invalid internal token: {err}")))?;
        let role = InternalRole::parse(&claims.custom.role)
            .ok_or_else(|| Status::permission_denied("unsupported internal role"))?;
        if let Some(expected) = self.expected_issuer.as_deref()
            && claims.custom.issuer.as_deref() != Some(expected)
        {
            return Err(Status::permission_denied("internal token issuer mismatch"));
        }
        if let Some(expected) = self.expected_audience.as_deref()
            && claims.custom.audience.as_deref() != Some(expected)
        {
            return Err(Status::permission_denied(
                "internal token audience mismatch",
            ));
        }
        let permissions = claims
            .custom
            .permissions
            .into_iter()
            .map(|value: String| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();
        Ok(InternalPrincipal {
            role,
            actor_id: claims.custom.actor_id,
            run_id: claims.custom.run_id,
            permissions,
        })
    }

    pub fn ensure_permission(
        &self,
        principal: &InternalPrincipal,
        action: InternalAction,
    ) -> Result<(), Status> {
        if principal.has_permission(action) {
            return Ok(());
        }
        Err(Status::permission_denied(format!(
            "internal permission denied: action '{}'",
            action.as_str()
        )))
    }

    pub fn ensure_run_scope(
        &self,
        principal: &InternalPrincipal,
        run_id: &str,
    ) -> Result<(), Status> {
        if let Some(scoped_run_id) = principal.run_id.as_deref()
            && scoped_run_id != run_id
        {
            return Err(Status::permission_denied(
                "internal token run scope mismatch",
            ));
        }
        Ok(())
    }

    pub fn ensure_worker_actor(
        &self,
        principal: &InternalPrincipal,
        actor_id: &str,
        field_name: &str,
    ) -> Result<(), Status> {
        if principal.role != InternalRole::Worker {
            return Ok(());
        }
        let expected_actor_id = principal
            .actor_id
            .as_deref()
            .ok_or_else(|| Status::permission_denied("worker token missing actor_id"))?;
        if expected_actor_id != actor_id {
            return Err(Status::permission_denied(format!(
                "worker token cannot access {field_name} '{actor_id}'"
            )));
        }
        Ok(())
    }

    pub fn issue_access_token(
        &self,
        role: InternalRole,
        actor_id: Option<&str>,
        run_id: Option<&str>,
        permissions: Vec<String>,
        ttl_seconds: i64,
    ) -> anyhow::Result<(String, i64)> {
        let ttl_seconds = ttl_seconds.clamp(60, 24 * 60 * 60);
        let claims = Claims::with_custom_claims(
            InternalAccessClaims {
                role: role.as_str().to_string(),
                actor_id: actor_id.map(|value| value.to_string()),
                run_id: run_id.map(|value| value.to_string()),
                permissions,
                issuer: self.expected_issuer.clone(),
                audience: self.expected_audience.clone(),
            },
            Duration::from_secs(ttl_seconds as u64),
        );
        let token = self
            .key
            .authenticate(claims)
            .map_err(|err| anyhow::anyhow!("issue internal token: {err}"))?;
        let expires_at = chrono::Utc::now().timestamp() + ttl_seconds;
        Ok((token, expires_at))
    }
}

fn bearer_token(metadata: &MetadataMap) -> Result<&str, Status> {
    let value = metadata
        .get(AUTHORIZATION_HEADER)
        .ok_or_else(|| Status::unauthenticated("missing authorization header"))?
        .to_str()
        .map_err(|_| Status::unauthenticated("invalid authorization header"))?;
    let token = value
        .strip_prefix(TOKEN_PREFIX)
        .ok_or_else(|| Status::unauthenticated("authorization header must use Bearer token"))?
        .trim();
    if token.is_empty() {
        return Err(Status::unauthenticated("authorization token is empty"));
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use jwt_simple::algorithms::MACLike;
    use jwt_simple::prelude::{Claims, Duration};
    use tonic::metadata::MetadataValue;

    use super::{
        HS256Key, InternalAccessClaims, InternalAction, InternalAuthz, InternalAuthzConfig,
        InternalRole,
    };
    const TEST_SECRET: &str = "agenthub-internal-test-secret";

    fn build_token(
        key: &HS256Key,
        role: &str,
        actor_id: Option<&str>,
        run_id: Option<&str>,
        permissions: Vec<&str>,
    ) -> String {
        let claims = Claims::with_custom_claims(
            InternalAccessClaims {
                role: role.to_string(),
                actor_id: actor_id.map(|value| value.to_string()),
                run_id: run_id.map(|value| value.to_string()),
                permissions: permissions.into_iter().map(str::to_string).collect(),
                issuer: Some("agenthub".to_string()),
                audience: Some("internal-grpc".to_string()),
            },
            Duration::from_hours(1),
        );
        key.authenticate(claims).expect("create token")
    }

    #[test]
    fn authenticate_worker_and_check_scopes() {
        let key = HS256Key::from_bytes(TEST_SECRET.as_bytes());
        let token = build_token(
            &key,
            "worker",
            Some("worker-1"),
            Some("run-1"),
            vec!["team:message:send", "team:inbox:list", "team:message:ack"],
        );
        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: TEST_SECRET.to_string(),
            expected_issuer: Some("agenthub".to_string()),
            expected_audience: Some("internal-grpc".to_string()),
        });

        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(
            "authorization",
            MetadataValue::try_from(format!("Bearer {token}")).expect("metadata value"),
        );
        let principal = authz.authenticate(&metadata).expect("authenticate");
        assert_eq!(principal.role, InternalRole::Worker);
        authz
            .ensure_permission(&principal, InternalAction::MessageSend)
            .expect("has send permission");
        authz
            .ensure_worker_actor(&principal, "worker-1", "from_actor_id")
            .expect("worker actor matches");
        authz
            .ensure_run_scope(&principal, "run-1")
            .expect("run scope matches");
        let actor_err = authz
            .ensure_worker_actor(&principal, "worker-2", "from_actor_id")
            .expect_err("worker actor mismatch should fail");
        assert_eq!(actor_err.code(), tonic::Code::PermissionDenied);
    }

    #[test]
    fn deny_missing_permission() {
        let key = HS256Key::from_bytes(TEST_SECRET.as_bytes());
        let token = build_token(&key, "leader", None, None, vec!["team:message:send"]);
        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: TEST_SECRET.to_string(),
            expected_issuer: Some("agenthub".to_string()),
            expected_audience: Some("internal-grpc".to_string()),
        });

        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(
            "authorization",
            MetadataValue::try_from(format!("Bearer {token}")).expect("metadata value"),
        );
        let principal = authz.authenticate(&metadata).expect("authenticate");
        let err = authz
            .ensure_permission(&principal, InternalAction::StepTransition)
            .expect_err("missing permission");
        assert_eq!(err.code(), tonic::Code::PermissionDenied);
    }

    #[test]
    fn reject_audience_mismatch() {
        let key = HS256Key::from_bytes(TEST_SECRET.as_bytes());
        let token = build_token(&key, "leader", None, None, vec!["*"]);
        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: TEST_SECRET.to_string(),
            expected_issuer: Some("agenthub".to_string()),
            expected_audience: Some("wrong-aud".to_string()),
        });
        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(
            "authorization",
            MetadataValue::try_from(format!("Bearer {token}")).expect("metadata value"),
        );
        let err = authz
            .authenticate(&metadata)
            .expect_err("audience mismatch");
        assert_eq!(err.code(), tonic::Code::PermissionDenied);
    }

    #[test]
    fn issue_access_token_round_trip() {
        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: TEST_SECRET.to_string(),
            expected_issuer: Some("agenthub".to_string()),
            expected_audience: Some("internal-grpc".to_string()),
        });
        let (token, _expires_at) = authz
            .issue_access_token(
                InternalRole::Worker,
                Some("worker-a"),
                Some("run-a"),
                vec![
                    InternalAction::MessageSend.as_str().to_string(),
                    InternalAction::InboxList.as_str().to_string(),
                ],
                600,
            )
            .expect("issue access token");

        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(
            "authorization",
            MetadataValue::try_from(format!("Bearer {token}")).expect("metadata value"),
        );
        let principal = authz.authenticate(&metadata).expect("authenticate");
        assert_eq!(principal.role, InternalRole::Worker);
        assert_eq!(principal.actor_id.as_deref(), Some("worker-a"));
        assert_eq!(principal.run_id.as_deref(), Some("run-a"));
        authz
            .ensure_permission(&principal, InternalAction::MessageSend)
            .expect("send permission exists");
    }
}
