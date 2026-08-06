use std::sync::Arc;

use sqlx::SqlitePool;
use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::*;

mod pending;

pub use agenthub_auth_domain::{LoginStartResult, RegisterStartResult, UserRecord};
use agenthub_auth_domain::{hash_password, verify_password};
use agenthub_auth_passkeys::PasskeyStore;
use agenthub_auth_store::AuthStore;
use pending::{PendingChallenge, PendingChallengeStore};

#[derive(Clone)]
pub struct AuthService {
    store: AuthStore,
    passkeys: PasskeyStore,
    webauthn: Arc<Webauthn>,
    pending: PendingChallengeStore,
    passkey_enabled: bool,
    rp_origin: String,
}

impl AuthService {
    pub fn rp_origin(&self) -> String {
        self.rp_origin.clone()
    }

    pub async fn new(db: SqlitePool, config: &agenthub_config::AppConfig) -> anyhow::Result<Self> {
        let rp_id = config.rp_id();
        let rp_origin = config.rp_origin();
        let rp_name = config.rp_name();
        let passkey_enabled = config.passkey_enabled();

        let rp_origin_url = Url::parse(&rp_origin)?;
        let builder = WebauthnBuilder::new(&rp_id, &rp_origin_url)?;
        let builder = builder.rp_name(&rp_name);
        let webauthn = builder.build()?;

        Ok(Self {
            store: AuthStore::new(db.clone()),
            passkeys: PasskeyStore::new(db),
            webauthn: Arc::new(webauthn),
            pending: PendingChallengeStore::default(),
            passkey_enabled,
            rp_origin,
        })
    }

    pub async fn is_passkey_enabled(&self) -> anyhow::Result<bool> {
        self.store.is_passkey_enabled(self.passkey_enabled).await
    }

    pub async fn set_passkey_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        self.store.set_passkey_enabled(enabled).await
    }

    pub async fn register_start(
        &self,
        username: &str,
        display_name: &str,
        role: &str,
        password: Option<&str>,
        device_name: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<RegisterStartResult> {
        let username = username.trim();
        let display_name = display_name.trim();
        if username.is_empty() {
            anyhow::bail!("username cannot be empty");
        }
        if username.contains('@') {
            anyhow::bail!("username cannot contain @ (reserved for mentions)");
        }
        if display_name.is_empty() {
            anyhow::bail!("display name cannot be empty");
        }
        if role == "root" && password.unwrap_or("").trim().is_empty() {
            anyhow::bail!("root user requires a password");
        }

        if let Some(existing) = self.get_user_by_username(username).await? {
            if existing.role != role {
                anyhow::bail!("username already exists");
            }
            if let Some(password) = password {
                let password_hash = hash_password(password)?;
                self.store
                    .update_user_password_hash(&existing.id, &password_hash)
                    .await?;
            }
            return self
                .start_registration_for_user(
                    &existing.id,
                    &existing.username,
                    &existing.display_name,
                    role,
                    device_name,
                    user_agent,
                )
                .await;
        }

        let user_id = Uuid::new_v4().to_string();
        let password_hash = if let Some(password) = password {
            Some(hash_password(password)?)
        } else {
            None
        };
        self.store
            .insert_user(
                &user_id,
                username,
                display_name,
                role,
                password_hash.as_deref(),
            )
            .await?;

        self.start_registration_for_user(
            &user_id,
            username,
            display_name,
            role,
            device_name,
            user_agent,
        )
        .await
    }
    async fn start_registration_for_user(
        &self,
        user_id: &str,
        username: &str,
        display_name: &str,
        role: &str,
        device_name: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<RegisterStartResult> {
        if !self.is_passkey_enabled().await? {
            if role == "device"
                && let Some(name) = device_name
            {
                let user_agent = user_agent.unwrap_or_else(|| "unknown".to_string());
                self.store
                    .insert_device(user_id, &name, &user_agent)
                    .await?;
            }
            return Ok(RegisterStartResult::Complete {
                user_id: user_id.to_string(),
                role: role.to_string(),
            });
        }
        let user_uuid = Uuid::parse_str(user_id)?;
        let (ccr, state) =
            self.webauthn
                .start_passkey_registration(user_uuid, username, display_name, None)?;

        let challenge_id = Uuid::new_v4().to_string();
        self.pending
            .insert_registration(
                challenge_id.clone(),
                user_id.to_string(),
                state,
                role.to_string(),
                device_name,
                user_agent,
            )
            .await;

        Ok(RegisterStartResult::Challenge {
            challenge_id,
            options: Box::new(ccr),
        })
    }

    pub async fn register_finish(
        &self,
        challenge_id: &str,
        cred: RegisterPublicKeyCredential,
    ) -> anyhow::Result<String> {
        let pending = self
            .pending
            .take(challenge_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("invalid challenge"))?;
        let PendingChallenge::Registration {
            user_id,
            state,
            role,
            device_name,
            user_agent,
        } = pending
        else {
            anyhow::bail!("invalid challenge type");
        };

        if !self.is_passkey_enabled().await? {
            anyhow::bail!("passkey registration is disabled");
        }

        let passkey = self.webauthn.finish_passkey_registration(&cred, &state)?;

        let mut passkeys = self.passkeys.load_passkeys(&user_id).await?;
        passkeys.push(passkey);
        self.passkeys.save_passkeys(&user_id, &passkeys).await?;

        if role == "device"
            && let Some(name) = device_name
        {
            let user_agent = user_agent.unwrap_or_else(|| "unknown".to_string());
            self.store
                .insert_device(&user_id, &name, &user_agent)
                .await?;
        }

        Ok(user_id)
    }

    pub async fn login_start(
        &self,
        username: &str,
        password: &str,
    ) -> anyhow::Result<LoginStartResult> {
        let username = username.trim();
        let password = password.trim();
        if username.is_empty() {
            anyhow::bail!("username cannot be empty");
        }
        if password.is_empty() {
            anyhow::bail!("password cannot be empty");
        }

        let user = self
            .get_user_by_username(username)
            .await?
            .ok_or_else(|| anyhow::anyhow!("user not found"))?;
        if let Some(ref hash) = user.password_hash {
            if !verify_password(password, hash)? {
                anyhow::bail!("invalid password");
            }
        } else {
            anyhow::bail!("password not set");
        }

        if !self.is_passkey_enabled().await? {
            return Ok(LoginStartResult::Complete {
                user_id: user.id,
                role: user.role,
            });
        }

        let passkeys = self.passkeys.load_passkeys(&user.id).await?;
        if passkeys.is_empty() {
            // No passkeys registered, but passkeys are enabled globally.
            // Let the user "join" by providing a registration challenge.
            return self
                .start_registration_for_user(
                    &user.id,
                    &user.username,
                    &user.display_name,
                    &user.role,
                    None,
                    None,
                )
                .await
                .map(|res| match res {
                    RegisterStartResult::Challenge {
                        challenge_id,
                        options,
                    } => LoginStartResult::Registration {
                        challenge_id,
                        options,
                        role: user.role,
                    },
                    RegisterStartResult::Complete { user_id, role } => {
                        LoginStartResult::Complete { user_id, role }
                    }
                });
        }

        let (rcr, state) = self.webauthn.start_passkey_authentication(&passkeys)?;
        let challenge_id = Uuid::new_v4().to_string();
        self.pending
            .insert_authentication(challenge_id.clone(), user.id, state)
            .await;

        Ok(LoginStartResult::Challenge {
            challenge_id,
            options: Box::new(rcr),
        })
    }

    pub async fn login_finish(
        &self,
        challenge_id: &str,
        cred: PublicKeyCredential,
    ) -> anyhow::Result<String> {
        let pending = self
            .pending
            .take(challenge_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("invalid challenge"))?;
        let PendingChallenge::Authentication { user_id, state } = pending else {
            anyhow::bail!("invalid challenge type");
        };

        if !self.is_passkey_enabled().await? {
            anyhow::bail!("passkey authentication is disabled");
        }

        let mut passkeys = self.passkeys.load_passkeys(&user_id).await?;
        let result = self.webauthn.finish_passkey_authentication(&cred, &state)?;
        let mut changed = false;
        for passkey in &mut passkeys {
            if let Some(updated) = passkey.update_credential(&result) {
                changed |= updated;
            }
        }
        if changed {
            self.passkeys.save_passkeys(&user_id, &passkeys).await?;
        }

        Ok(user_id)
    }

    pub async fn create_session(&self, user_id: &str) -> anyhow::Result<String> {
        self.store.create_session(user_id).await
    }

    pub async fn validate_session(&self, token: &str) -> anyhow::Result<UserRecord> {
        self.store.validate_session(token).await
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> anyhow::Result<UserRecord> {
        self.store.get_user_by_id(user_id).await
    }

    async fn get_user_by_username(&self, username: &str) -> anyhow::Result<Option<UserRecord>> {
        self.store.get_user_by_username(username).await
    }
    pub async fn record_audit(
        &self,
        user_id: Option<&str>,
        device_id: Option<&str>,
        event: &str,
        detail: Option<&str>,
        ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> anyhow::Result<()> {
        self.store
            .record_audit(user_id, device_id, event, detail, ip, user_agent)
            .await
    }

    pub async fn get_device_id_for_user(&self, user_id: &str) -> anyhow::Result<Option<String>> {
        self.store.get_device_id_for_user(user_id).await
    }

    pub async fn touch_device_login(&self, user_id: &str) -> anyhow::Result<()> {
        self.store.touch_device_login(user_id).await
    }

    pub async fn root_exists(&self) -> anyhow::Result<bool> {
        self.store.root_exists().await
    }
}
