use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand_core::OsRng;
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse};

#[derive(Debug)]
pub enum RegisterStartResult {
    Challenge {
        challenge_id: String,
        options: Box<CreationChallengeResponse>,
    },
    Complete {
        user_id: String,
        role: String,
    },
}

#[derive(Debug)]
pub enum LoginStartResult {
    Challenge {
        challenge_id: String,
        options: Box<RequestChallengeResponse>,
    },
    Registration {
        challenge_id: String,
        options: Box<CreationChallengeResponse>,
        role: String,
    },
    Complete {
        user_id: String,
        role: String,
    },
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password_hash: Option<String>,
}

impl UserRecord {
    pub fn has_capability(&self, capability: UserCapability) -> bool {
        user_has_capability(&self.role, capability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserRole {
    Root,
    Admin,
    Operator,
    Viewer,
    Device,
}

impl UserRole {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "root" => Some(Self::Root),
            "admin" => Some(Self::Admin),
            "operator" => Some(Self::Operator),
            "viewer" => Some(Self::Viewer),
            "device" => Some(Self::Device),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
            Self::Device => "device",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserCapability {
    InstanceConfigure,
    UsersManage,
    AuthManage,
    AgentsManage,
    TeamsManage,
    NodesManage,
    LinkersManage,
    RuntimeOperate,
    RuntimeInspect,
    DiagnosticsRead,
    PushSubscribe,
}

impl UserCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InstanceConfigure => "instance:configure",
            Self::UsersManage => "users:manage",
            Self::AuthManage => "auth:manage",
            Self::AgentsManage => "agents:manage",
            Self::TeamsManage => "teams:manage",
            Self::NodesManage => "nodes:manage",
            Self::LinkersManage => "linkers:manage",
            Self::RuntimeOperate => "runtime:operate",
            Self::RuntimeInspect => "runtime:inspect",
            Self::DiagnosticsRead => "diagnostics:read",
            Self::PushSubscribe => "push:subscribe",
        }
    }
}

pub fn user_has_capability(role: &str, capability: UserCapability) -> bool {
    let Some(role) = UserRole::parse(role) else {
        return false;
    };
    role_has_capability(role, capability)
}

pub fn role_has_capability(role: UserRole, capability: UserCapability) -> bool {
    use UserCapability as Capability;
    use UserRole as Role;

    match role {
        Role::Root => true,
        Role::Admin => matches!(
            capability,
            Capability::AgentsManage
                | Capability::TeamsManage
                | Capability::NodesManage
                | Capability::LinkersManage
                | Capability::RuntimeOperate
                | Capability::RuntimeInspect
                | Capability::DiagnosticsRead
                | Capability::PushSubscribe
        ),
        Role::Operator => matches!(
            capability,
            Capability::AgentsManage
                | Capability::TeamsManage
                | Capability::RuntimeOperate
                | Capability::RuntimeInspect
                | Capability::PushSubscribe
        ),
        Role::Viewer => matches!(
            capability,
            Capability::RuntimeInspect | Capability::PushSubscribe
        ),
        Role::Device => matches!(capability, Capability::PushSubscribe),
    }
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let argon2 = Argon2::default();
    Ok(argon2.verify_password(password.as_bytes(), &parsed).is_ok())
}

#[cfg(test)]
mod tests {
    use super::{UserCapability, UserRecord, UserRole, role_has_capability, user_has_capability};

    const CAPABILITIES: [UserCapability; 11] = [
        UserCapability::InstanceConfigure,
        UserCapability::UsersManage,
        UserCapability::AuthManage,
        UserCapability::AgentsManage,
        UserCapability::TeamsManage,
        UserCapability::NodesManage,
        UserCapability::LinkersManage,
        UserCapability::RuntimeOperate,
        UserCapability::RuntimeInspect,
        UserCapability::DiagnosticsRead,
        UserCapability::PushSubscribe,
    ];

    #[test]
    fn user_role_capability_matrix_is_explicit() {
        let cases = [
            (
                UserRole::Root,
                [
                    true, true, true, true, true, true, true, true, true, true, true,
                ],
            ),
            (
                UserRole::Admin,
                [
                    false, false, false, true, true, true, true, true, true, true, true,
                ],
            ),
            (
                UserRole::Operator,
                [
                    false, false, false, true, true, false, false, true, true, false, true,
                ],
            ),
            (
                UserRole::Viewer,
                [
                    false, false, false, false, false, false, false, false, true, false, true,
                ],
            ),
            (
                UserRole::Device,
                [
                    false, false, false, false, false, false, false, false, false, false, true,
                ],
            ),
        ];

        for (role, expected) in cases {
            for (capability, allowed) in CAPABILITIES.into_iter().zip(expected) {
                assert_eq!(
                    role_has_capability(role, capability),
                    allowed,
                    "role={} capability={}",
                    role.as_str(),
                    capability.as_str()
                );
                assert_eq!(
                    user_has_capability(role.as_str(), capability),
                    allowed,
                    "raw role={} capability={}",
                    role.as_str(),
                    capability.as_str()
                );
            }
        }
    }

    #[test]
    fn unknown_user_role_denies_every_capability() {
        for capability in CAPABILITIES {
            assert!(
                !user_has_capability("unknown", capability),
                "capability={}",
                capability.as_str()
            );
        }
    }

    #[test]
    fn user_record_delegates_capability_checks_to_role_matrix() {
        let user = UserRecord {
            id: "user-1".to_string(),
            username: "operator".to_string(),
            display_name: "Operator".to_string(),
            role: "operator".to_string(),
            password_hash: None,
        };

        assert!(user.has_capability(UserCapability::AgentsManage));
        assert!(!user.has_capability(UserCapability::NodesManage));
    }
}
