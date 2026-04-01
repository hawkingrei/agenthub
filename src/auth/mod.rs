use std::collections::HashMap;
use std::sync::Arc;

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::*;

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

#[derive(Clone)]
pub struct AuthService {
    db: SqlitePool,
    webauthn: Option<Arc<Webauthn>>,
    pending: Arc<RwLock<HashMap<String, PendingChallenge>>>,
    passkey_enabled: bool,
}

#[derive(Debug)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password_hash: Option<String>,
}

#[derive(Debug)]
enum PendingChallenge {
    Registration {
        user_id: String,
        state: PasskeyRegistration,
        role: String,
        device_name: Option<String>,
        user_agent: Option<String>,
    },
    Authentication {
        user_id: String,
        state: PasskeyAuthentication,
    },
}

impl AuthService {
    pub async fn new(db: SqlitePool, config: &agenthub_config::AppConfig) -> anyhow::Result<Self> {
        let rp_id = config.rp_id();
        let rp_origin = config.rp_origin();
        let rp_name = config.rp_name();
        let passkey_enabled = config.passkey_enabled();

        let rp_origin = Url::parse(&rp_origin)?;
        let builder = WebauthnBuilder::new(&rp_id, &rp_origin)?;
        let builder = builder.rp_name(&rp_name);
        let webauthn = builder.build()?;

        Ok(Self {
            db,
            webauthn: Some(Arc::new(webauthn)),
            pending: Arc::new(RwLock::new(HashMap::new())),
            passkey_enabled,
        })
    }

    pub async fn is_passkey_enabled(&self) -> anyhow::Result<bool> {
        let row = sqlx::query("SELECT value FROM system_config WHERE key = 'passkey_enabled'")
            .fetch_optional(&self.db)
            .await?;

        if let Some(row) = row {
            let val: String = row.get("value");
            Ok(val == "true")
        } else {
            Ok(self.passkey_enabled)
        }
    }

    pub async fn set_passkey_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_config (key, value)
            VALUES ('passkey_enabled', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
        )
        .bind(if enabled { "true" } else { "false" })
        .execute(&self.db)
        .await?;
        Ok(())
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
        if let Some(existing) = self.get_user_by_username(username).await? {
            if existing.role != role {
                anyhow::bail!("username already exists");
            }
            if let Some(password) = password {
                let password_hash = hash_password(password)?;
                sqlx::query(
                    r#"
                    UPDATE users
                    SET password_hash = ?1
                    WHERE id = ?2
                    "#,
                )
                .bind(password_hash)
                .bind(&existing.id)
                .execute(&self.db)
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
        let now = Utc::now().timestamp();
        let password_hash = if let Some(password) = password {
            Some(hash_password(password)?)
        } else {
            None
        };
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&user_id)
        .bind(username)
        .bind(display_name)
        .bind(role)
        .bind(password_hash)
        .bind(now)
        .execute(&self.db)
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
                self.insert_device(user_id, &name, &user_agent).await?;
            }
            return Ok(RegisterStartResult::Complete {
                user_id: user_id.to_string(),
                role: role.to_string(),
            });
        }
        let webauthn = self
            .webauthn
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("webauthn not initialized"))?;
        let user_uuid = Uuid::parse_str(user_id)?;
        let (ccr, state) =
            webauthn.start_passkey_registration(user_uuid, username, display_name, None)?;

        let challenge_id = Uuid::new_v4().to_string();
        let mut pending = self.pending.write().await;
        pending.insert(
            challenge_id.clone(),
            PendingChallenge::Registration {
                user_id: user_id.to_string(),
                state,
                role: role.to_string(),
                device_name,
                user_agent,
            },
        );

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
        let pending = {
            let mut guard = self.pending.write().await;
            guard.remove(challenge_id)
        };

        let pending = pending.ok_or_else(|| anyhow::anyhow!("invalid challenge"))?;
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

        let webauthn = self
            .webauthn
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("webauthn not initialized"))?;
        let passkey = webauthn.finish_passkey_registration(&cred, &state)?;

        let mut passkeys = self.load_passkeys(&user_id).await?;
        passkeys.push(passkey);
        self.save_passkeys(&user_id, &passkeys).await?;

        if role == "device"
            && let Some(name) = device_name
        {
            let user_agent = user_agent.unwrap_or_else(|| "unknown".to_string());
            self.insert_device(&user_id, &name, &user_agent).await?;
        }

        Ok(user_id)
    }

    pub async fn login_start(
        &self,
        username: &str,
        password: &str,
    ) -> anyhow::Result<LoginStartResult> {
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

        let webauthn = self
            .webauthn
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("webauthn not initialized"))?;
        let passkeys = self.load_passkeys(&user.id).await?;
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

        let (rcr, state) = webauthn.start_passkey_authentication(&passkeys)?;
        let challenge_id = Uuid::new_v4().to_string();
        let mut pending = self.pending.write().await;
        pending.insert(
            challenge_id.clone(),
            PendingChallenge::Authentication {
                user_id: user.id,
                state,
            },
        );

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
        let pending = {
            let mut guard = self.pending.write().await;
            guard.remove(challenge_id)
        };

        let pending = pending.ok_or_else(|| anyhow::anyhow!("invalid challenge"))?;
        let PendingChallenge::Authentication { user_id, state } = pending else {
            anyhow::bail!("invalid challenge type");
        };

        if !self.is_passkey_enabled().await? {
            anyhow::bail!("passkey authentication is disabled");
        }

        let webauthn = self
            .webauthn
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("webauthn not initialized"))?;

        let mut passkeys = self.load_passkeys(&user_id).await?;
        let result = webauthn.finish_passkey_authentication(&cred, &state)?;
        let mut changed = false;
        for passkey in &mut passkeys {
            if let Some(updated) = passkey.update_credential(&result) {
                changed |= updated;
            }
        }
        if changed {
            self.save_passkeys(&user_id, &passkeys).await?;
        }

        Ok(user_id)
    }

    pub async fn create_session(&self, user_id: &str) -> anyhow::Result<String> {
        let token = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let expires_at = (Utc::now() + Duration::hours(12)).timestamp();

        sqlx::query(
            r#"
            INSERT INTO auth_sessions (token, user_id, created_at, expires_at, revoked_at)
            VALUES (?1, ?2, ?3, ?4, NULL)
            "#,
        )
        .bind(&token)
        .bind(user_id)
        .bind(now)
        .bind(expires_at)
        .execute(&self.db)
        .await?;

        Ok(token)
    }

    pub async fn validate_session(&self, token: &str) -> anyhow::Result<UserRecord> {
        let row = sqlx::query(
            r#"
            SELECT u.id, u.username, u.display_name, u.role, u.password_hash
            FROM auth_sessions s
            JOIN users u ON s.user_id = u.id
            WHERE s.token = ?1 AND s.expires_at > ?2 AND s.revoked_at IS NULL
            "#,
        )
        .bind(token)
        .bind(Utc::now().timestamp())
        .fetch_one(&self.db)
        .await?;

        let user = UserRecord {
            id: row.get("id"),
            username: row.get("username"),
            display_name: row.get("display_name"),
            role: row.get("role"),
            password_hash: row.get("password_hash"),
        };

        if user.role == "device" {
            let active = sqlx::query(
                r#"
                SELECT id FROM devices
                WHERE user_id = ?1 AND status = 'active'
                LIMIT 1
                "#,
            )
            .bind(&user.id)
            .fetch_optional(&self.db)
            .await?;
            if active.is_none() {
                anyhow::bail!("device revoked");
            }
        }

        Ok(user)
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> anyhow::Result<UserRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, username, display_name, role, password_hash
            FROM users
            WHERE id = ?1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await?;

        Ok(UserRecord {
            id: row.get("id"),
            username: row.get("username"),
            display_name: row.get("display_name"),
            role: row.get("role"),
            password_hash: row.get("password_hash"),
        })
    }

    async fn get_user_by_username(&self, username: &str) -> anyhow::Result<Option<UserRecord>> {
        let row = sqlx::query(
            r#"
            SELECT id, username, display_name, role, password_hash
            FROM users
            WHERE username = ?1
            "#,
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|r| UserRecord {
            id: r.get("id"),
            username: r.get("username"),
            display_name: r.get("display_name"),
            role: r.get("role"),
            password_hash: r.get("password_hash"),
        }))
    }

    async fn load_passkeys(&self, user_id: &str) -> anyhow::Result<Vec<Passkey>> {
        let row = sqlx::query(
            r#"
            SELECT passkeys
            FROM user_passkeys
            WHERE user_id = ?1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some(row) = row {
            let json: String = row.get("passkeys");
            let passkeys = serde_json::from_str::<Vec<Passkey>>(&json)?;
            Ok(passkeys)
        } else {
            Ok(Vec::new())
        }
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
        let ts = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO login_audit (user_id, device_id, event, ip, user_agent, detail, ts)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(user_id)
        .bind(device_id)
        .bind(event)
        .bind(ip)
        .bind(user_agent)
        .bind(detail)
        .bind(ts)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn get_device_id_for_user(&self, user_id: &str) -> anyhow::Result<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT id
            FROM devices
            WHERE user_id = ?1 AND status = 'active'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|r| r.get("id")))
    }

    pub async fn touch_device_login(&self, user_id: &str) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE devices
            SET last_login_at = ?1
            WHERE user_id = ?2
            "#,
        )
        .bind(now)
        .bind(user_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn root_has_passkeys(&self) -> anyhow::Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT u.id
            FROM users u
            JOIN user_passkeys p ON u.id = p.user_id
            WHERE u.role = 'root'
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(row.is_some())
    }

    async fn save_passkeys(&self, user_id: &str, passkeys: &[Passkey]) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let json = serde_json::to_string(passkeys)?;
        sqlx::query(
            r#"
            INSERT INTO user_passkeys (user_id, passkeys, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(user_id) DO UPDATE SET
                passkeys = excluded.passkeys,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(user_id)
        .bind(json)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn insert_device(
        &self,
        user_id: &str,
        name: &str,
        user_agent: &str,
    ) -> anyhow::Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO devices (id, user_id, name, user_agent, status, created_at)
            VALUES (?1, ?2, ?3, ?4, 'active', ?5)
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(name)
        .bind(user_agent)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .to_string();
    Ok(hash)
}

fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let argon2 = Argon2::default();
    Ok(argon2.verify_password(password.as_bytes(), &parsed).is_ok())
}
