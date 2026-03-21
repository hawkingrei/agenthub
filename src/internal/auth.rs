use std::collections::HashSet;

use jwt_simple::algorithms::MACLike;
use jwt_simple::prelude::{Claims, Duration, HS256Key};
use serde::{Deserialize, Serialize};
use tonic::{Status, metadata::MetadataMap};

use super::p2p::{
    CredentialProvider, IssuedNodeAccessToken, NodeCredentialRequest, derive_cluster_id,
    normalize_audience, normalize_scope, shared_key_kid,
};
use crate::agent::AGENT_NODE_MAIN_ID;

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
    #[serde(default)]
    pub cluster_id: Option<String>,
    #[serde(default)]
    pub source_node_id: Option<String>,
    pub actor_id: Option<String>,
    pub run_id: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub scope: Vec<String>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
    #[serde(default)]
    pub kid: Option<String>,
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

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InternalPrincipal {
    pub role: InternalRole,
    pub cluster_id: Option<String>,
    pub source_node_id: Option<String>,
    pub actor_id: Option<String>,
    pub run_id: Option<String>,
    pub scope: HashSet<String>,
    pub audience: Option<String>,
    pub kid: Option<String>,
    pub issued_at: Option<i64>,
    pub expires_at: Option<i64>,
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
    AgentManage,
}

impl InternalAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MessageSend => "team:message:send",
            Self::InboxList => "team:inbox:list",
            Self::MessageAck => "team:message:ack",
            Self::StepTransition => "team:step:transition",
            Self::NodeIssue => "team:node:issue",
            Self::AgentManage => "agent:manage",
        }
    }
}

#[derive(Clone)]
pub struct InternalAuthz {
    key: HS256Key,
    expected_issuer: Option<String>,
    expected_audience: Option<String>,
    cluster_id: String,
    key_id: String,
}

impl InternalAuthz {
    pub fn new(config: InternalAuthzConfig) -> Self {
        let cluster_id = derive_cluster_id(config.expected_issuer.as_deref());
        let key_id = shared_key_kid(&config.shared_secret);
        Self {
            key: HS256Key::from_bytes(config.shared_secret.as_bytes()),
            expected_issuer: config.expected_issuer,
            expected_audience: config.expected_audience,
            cluster_id,
            key_id,
        }
    }

    pub fn authenticate(&self, metadata: &MetadataMap) -> Result<InternalPrincipal, Status> {
        let token = bearer_token(metadata)?;
        let claims = self
            .key
            .verify_token::<InternalAccessClaims>(token, None)
            .map_err(|err| Status::unauthenticated(format!("invalid internal token: {err}")))?;
        let issued_at = claims.issued_at.map(|value| value.as_secs() as i64);
        let expires_at = claims.expires_at.map(|value| value.as_secs() as i64);
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
        let scope = claims
            .custom
            .scope
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();
        Ok(InternalPrincipal {
            role,
            cluster_id: claims.custom.cluster_id,
            source_node_id: claims.custom.source_node_id,
            actor_id: claims.custom.actor_id,
            run_id: claims.custom.run_id,
            scope,
            audience: claims.custom.audience,
            kid: claims.custom.kid,
            issued_at,
            expires_at,
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

    #[allow(dead_code)]
    pub fn issue_access_token(
        &self,
        role: InternalRole,
        actor_id: Option<&str>,
        run_id: Option<&str>,
        permissions: Vec<String>,
        ttl_seconds: i64,
    ) -> anyhow::Result<(String, i64)> {
        let issued = self.issue_node_access_token(NodeCredentialRequest {
            source_node_id: AGENT_NODE_MAIN_ID.to_string(),
            role: role.as_str().to_string(),
            actor_id: actor_id.map(str::to_string),
            run_id: run_id.map(str::to_string),
            permissions,
            scope: Vec::new(),
            audience: Vec::new(),
            ttl_seconds,
        })?;
        Ok((issued.access_token, issued.expires_at))
    }

    #[allow(dead_code)]
    pub fn cluster_id(&self) -> &str {
        &self.cluster_id
    }

    #[allow(dead_code)]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
}

impl CredentialProvider for InternalAuthz {
    fn issue_node_access_token(
        &self,
        request: NodeCredentialRequest,
    ) -> anyhow::Result<IssuedNodeAccessToken> {
        let ttl_seconds = request.ttl_seconds.clamp(60, 24 * 60 * 60);
        InternalRole::parse(&request.role)
            .ok_or_else(|| anyhow::anyhow!("unsupported internal role '{}'", request.role))?;
        let scope = normalize_scope(request.scope, &request.permissions);
        let audience = normalize_audience(request.audience, self.expected_audience.as_deref());
        let token_audience = self
            .expected_audience
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| audience.first().cloned());
        let issued_at = chrono::Utc::now().timestamp();
        let expires_at = issued_at + ttl_seconds;
        let claims = Claims::with_custom_claims(
            InternalAccessClaims {
                role: request.role.clone(),
                cluster_id: Some(self.cluster_id.clone()),
                source_node_id: Some(request.source_node_id.clone()),
                actor_id: request.actor_id,
                run_id: request.run_id,
                permissions: request.permissions,
                scope: scope.clone(),
                issuer: self.expected_issuer.clone(),
                audience: token_audience.clone(),
                kid: Some(self.key_id.clone()),
            },
            Duration::from_secs(ttl_seconds as u64),
        );
        let token = self
            .key
            .authenticate(claims)
            .map_err(|err| anyhow::anyhow!("issue internal token: {err}"))?;
        Ok(IssuedNodeAccessToken {
            source_node_id: request.source_node_id,
            role: request.role,
            access_token: token,
            cluster_id: self.cluster_id.clone(),
            scope,
            audience: token_audience.into_iter().collect(),
            kid: self.key_id.clone(),
            issued_at,
            expires_at,
        })
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

    use crate::internal::p2p::CredentialProvider;

    use super::{
        HS256Key, InternalAccessClaims, InternalAction, InternalAuthz, InternalAuthzConfig,
        InternalRole, NodeCredentialRequest,
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
                cluster_id: Some("agenthub".to_string()),
                source_node_id: Some("node-test".to_string()),
                actor_id: actor_id.map(|value| value.to_string()),
                run_id: run_id.map(|value| value.to_string()),
                permissions: permissions.into_iter().map(str::to_string).collect(),
                scope: vec!["node:p2p".to_string()],
                issuer: Some("agenthub".to_string()),
                audience: Some("internal-grpc".to_string()),
                kid: Some("shared-hs256-test".to_string()),
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
        assert_eq!(principal.cluster_id.as_deref(), Some("agenthub"));
        assert_eq!(principal.source_node_id.as_deref(), Some("node-test"));
        assert!(principal.scope.contains("node:p2p"));
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
        assert_eq!(principal.cluster_id.as_deref(), Some("agenthub"));
        assert_eq!(principal.source_node_id.as_deref(), Some("main"));
        assert_eq!(principal.audience.as_deref(), Some("internal-grpc"));
        assert!(principal.scope.contains("node:p2p"));
        assert_eq!(principal.kid.as_deref(), Some(authz.key_id()));
        assert!(principal.issued_at.is_some());
        assert!(principal.expires_at.is_some());
        authz
            .ensure_permission(&principal, InternalAction::MessageSend)
            .expect("send permission exists");
    }

    #[test]
    fn issue_node_access_token_prefers_expected_audience_for_claims_and_metadata() {
        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: TEST_SECRET.to_string(),
            expected_issuer: Some("agenthub".to_string()),
            expected_audience: Some("internal-grpc".to_string()),
        });
        let issued = authz
            .issue_node_access_token(NodeCredentialRequest {
                source_node_id: "node-a".to_string(),
                role: InternalRole::Leader.as_str().to_string(),
                actor_id: None,
                run_id: None,
                permissions: vec![InternalAction::AgentManage.as_str().to_string()],
                scope: Vec::new(),
                audience: vec!["secondary-audience".to_string()],
                ttl_seconds: 600,
            })
            .expect("issue node access token");
        assert_eq!(issued.audience, vec!["internal-grpc".to_string()]);

        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(
            "authorization",
            MetadataValue::try_from(format!("Bearer {}", issued.access_token))
                .expect("metadata value"),
        );
        let principal = authz.authenticate(&metadata).expect("authenticate");
        assert_eq!(principal.audience.as_deref(), Some("internal-grpc"));
    }

    #[test]
    fn issue_node_access_token_uses_primary_normalized_request_audience_without_expected() {
        let authz = InternalAuthz::new(InternalAuthzConfig {
            shared_secret: TEST_SECRET.to_string(),
            expected_issuer: Some("agenthub".to_string()),
            expected_audience: None,
        });
        let issued = authz
            .issue_node_access_token(NodeCredentialRequest {
                source_node_id: "node-a".to_string(),
                role: InternalRole::Leader.as_str().to_string(),
                actor_id: None,
                run_id: None,
                permissions: vec![InternalAction::AgentManage.as_str().to_string()],
                scope: Vec::new(),
                audience: vec![
                    "zeta-audience".to_string(),
                    "alpha-audience".to_string(),
                    "zeta-audience".to_string(),
                ],
                ttl_seconds: 600,
            })
            .expect("issue node access token");
        assert_eq!(issued.audience, vec!["alpha-audience".to_string()]);

        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert(
            "authorization",
            MetadataValue::try_from(format!("Bearer {}", issued.access_token))
                .expect("metadata value"),
        );
        let principal = authz.authenticate(&metadata).expect("authenticate");
        assert_eq!(principal.audience.as_deref(), Some("alpha-audience"));
    }
}
