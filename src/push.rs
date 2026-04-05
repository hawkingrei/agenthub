use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use jwt_simple::prelude::ES256KeyPair;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::sync::RwLock;
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient,
    WebPushMessageBuilder,
};

pub struct PushService {
    db: SqlitePool,
    subject: Option<String>,
    keys_path: Option<PathBuf>,
    keys: RwLock<Option<VapidKeys>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushSubscription {
    pub endpoint: String,
    pub keys: PushKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushKeys {
    pub p256dh: String,
    pub auth: String,
}

impl PushService {
    pub fn new(db: SqlitePool, config: &agenthub_config::AppConfig) -> anyhow::Result<Self> {
        let subject = config.vapid_subject();
        let keys_path = config.vapid_keys_path();
        let keys = load_or_create_vapid_keys(&keys_path)?;
        Ok(Self {
            db,
            subject: Some(subject),
            keys_path: Some(keys_path),
            keys: RwLock::new(Some(keys)),
        })
    }

    pub fn disabled(db: SqlitePool) -> Self {
        Self {
            db,
            subject: None,
            keys_path: None,
            keys: RwLock::new(None),
        }
    }

    pub fn public_key(&self) -> String {
        self.keys
            .read()
            .expect("vapid keys lock poisoned")
            .as_ref()
            .map(|keys| keys.public_key.clone())
            .unwrap_or_default()
    }

    pub fn is_enabled(&self) -> bool {
        self.keys
            .read()
            .expect("vapid keys lock poisoned")
            .is_some()
    }

    fn ensure_enabled(&self) -> anyhow::Result<()> {
        if self.is_enabled() {
            return Ok(());
        }
        anyhow::bail!("push notifications are disabled");
    }

    fn current_private_key(&self) -> anyhow::Result<String> {
        self.keys
            .read()
            .expect("vapid keys lock poisoned")
            .as_ref()
            .map(|keys| keys.private_key.clone())
            .ok_or_else(|| anyhow::anyhow!("push notifications are disabled"))
    }

    fn configured_subject(&self) -> anyhow::Result<&str> {
        self.subject
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("push notifications are disabled"))
    }

    fn configured_keys_path(&self) -> anyhow::Result<&std::path::Path> {
        self.keys_path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("push notifications are disabled"))
    }

    pub fn subject(&self) -> &str {
        self.subject.as_deref().unwrap_or("")
    }

    pub fn keys_path(&self) -> &std::path::Path {
        self.keys_path
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new(""))
    }

    pub fn rotate_keys(&self) -> anyhow::Result<String> {
        self.ensure_enabled()?;
        let keys_path = self.configured_keys_path()?.to_path_buf();
        let keys = generate_vapid_keys()?;
        let payload = serde_json::to_string_pretty(&keys)?;
        std::fs::write(&keys_path, payload)?;
        let public_key = keys.public_key.clone();
        let mut guard = self.keys.write().expect("vapid keys lock poisoned");
        *guard = Some(keys);
        Ok(public_key)
    }

    pub async fn save_subscription(
        &self,
        user_id: &str,
        sub: PushSubscription,
    ) -> anyhow::Result<()> {
        self.ensure_enabled()?;
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(user_id)
        .bind(&sub.endpoint)
        .bind(&sub.keys.p256dh)
        .bind(&sub.keys.auth)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn notify_agent_completed(
        &self,
        agent_id: &str,
        session_id: &str,
    ) -> anyhow::Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let payload = serde_json::to_vec(&serde_json::json!({
            "type": "agent_completed",
            "agent_id": agent_id,
            "session_id": session_id,
            "ts": Utc::now().timestamp(),
        }))?;

        let rows = sqlx::query(
            r#"
            SELECT endpoint, p256dh, auth
            FROM push_subscriptions
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        let client = IsahcWebPushClient::new()?;
        let private_key = self.current_private_key()?;
        let subject = self.configured_subject()?.to_string();
        for row in rows {
            let endpoint: String = row.get("endpoint");
            let p256dh: String = row.get("p256dh");
            let auth: String = row.get("auth");
            let subscription = SubscriptionInfo::new(endpoint, p256dh, auth);
            let mut sig_builder = VapidSignatureBuilder::from_base64(&private_key, &subscription)?;
            sig_builder.add_claim("sub", subject.as_str());
            let sig = sig_builder.build()?;

            let mut builder = WebPushMessageBuilder::new(&subscription);
            builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_slice());
            builder.set_vapid_signature(sig);

            let _ = client.send(builder.build()?).await;
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct VapidKeysFile {
    public_key: String,
    private_key: String,
}

type VapidKeys = VapidKeysFile;

fn load_or_create_vapid_keys(path: &std::path::Path) -> anyhow::Result<VapidKeys> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        let parsed: VapidKeysFile = serde_json::from_str(&content)?;
        if parsed.public_key.is_empty() || parsed.private_key.is_empty() {
            anyhow::bail!("vapid keys file missing required fields");
        }
        return Ok(parsed);
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let keys = generate_vapid_keys()?;
    let payload = serde_json::to_string_pretty(&keys)?;
    std::fs::write(path, payload)?;
    Ok(keys)
}

fn generate_vapid_keys() -> anyhow::Result<VapidKeys> {
    let key_pair = ES256KeyPair::generate();
    let private_bytes = key_pair.to_bytes();
    let private_key = URL_SAFE_NO_PAD.encode(&private_bytes);
    let builder = VapidSignatureBuilder::from_base64_no_sub(&private_key)?;
    let public_key = URL_SAFE_NO_PAD.encode(builder.get_public_key());
    Ok(VapidKeysFile {
        public_key,
        private_key,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        PushKeys, PushService, PushSubscription, VapidKeysFile, load_or_create_vapid_keys,
    };
    use agenthub_config::{AppConfig, PushConfig};
    use sqlx::Row;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use uuid::Uuid;

    async fn test_db() -> sqlx::SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite");
        sqlx::query(
            r#"
            CREATE TABLE push_subscriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                p256dh TEXT NOT NULL,
                auth TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create push_subscriptions table");
        pool
    }

    fn test_push_config(keys_path: &std::path::Path) -> AppConfig {
        AppConfig {
            push: Some(PushConfig {
                subject: Some("mailto:test@example.com".to_string()),
                keys_path: Some(keys_path.to_string_lossy().to_string()),
            }),
            ..Default::default()
        }
    }

    fn test_push_subscription() -> PushSubscription {
        PushSubscription {
            endpoint: "https://example.com/push".to_string(),
            keys: PushKeys {
                p256dh: "p256dh-test".to_string(),
                auth: "auth-test".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn disabled_push_service_exposes_empty_state_and_rejects_mutations() {
        let db = test_db().await;
        let push = PushService::disabled(db);

        assert!(!push.is_enabled());
        assert_eq!(push.public_key(), "");
        assert_eq!(push.subject(), "");
        assert!(push.keys_path().as_os_str().is_empty());
        assert_eq!(
            push.current_private_key()
                .expect_err("disabled service should not expose a private key")
                .to_string(),
            "push notifications are disabled"
        );
        assert_eq!(
            push.configured_subject()
                .expect_err("disabled service should not expose a subject")
                .to_string(),
            "push notifications are disabled"
        );
        assert_eq!(
            push.configured_keys_path()
                .expect_err("disabled service should not expose a keys path")
                .to_string(),
            "push notifications are disabled"
        );
        assert_eq!(
            push.rotate_keys()
                .expect_err("disabled service should not rotate keys")
                .to_string(),
            "push notifications are disabled"
        );
        assert_eq!(
            push.save_subscription("user-1", test_push_subscription())
                .await
                .expect_err("disabled service should reject subscriptions")
                .to_string(),
            "push notifications are disabled"
        );
        push.notify_agent_completed("agent-1", "session-1")
            .await
            .expect("disabled service should short-circuit notifications");
    }

    #[tokio::test]
    async fn push_service_creates_loads_and_rotates_vapid_keys() {
        let db = test_db().await;
        let temp_root = std::env::temp_dir().join(format!("agenthub-push-{}", Uuid::new_v4()));
        let keys_path = temp_root.join("nested").join("vapid.json");
        let config = test_push_config(&keys_path);

        let push = PushService::new(db.clone(), &config).expect("create push service");
        let initial_public_key = push.public_key();
        assert!(push.is_enabled());
        assert_eq!(push.subject(), "mailto:test@example.com");
        assert_eq!(
            push.configured_subject().expect("configured subject"),
            "mailto:test@example.com"
        );
        assert_eq!(
            push.configured_keys_path().expect("configured keys path"),
            keys_path.as_path()
        );
        assert!(!push.current_private_key().expect("private key").is_empty());
        assert!(keys_path.exists(), "new service should create VAPID keys");

        let created_keys: VapidKeysFile =
            serde_json::from_str(&std::fs::read_to_string(&keys_path).expect("read created keys"))
                .expect("parse created keys");
        assert_eq!(created_keys.public_key, initial_public_key);

        let reloaded = PushService::new(db, &config).expect("reload push service");
        assert_eq!(
            reloaded.public_key(),
            initial_public_key,
            "reloading should reuse existing keys"
        );

        let rotated_public_key = push.rotate_keys().expect("rotate keys");
        assert_ne!(rotated_public_key, initial_public_key);
        let rotated_keys: VapidKeysFile =
            serde_json::from_str(&std::fs::read_to_string(&keys_path).expect("read rotated keys"))
                .expect("parse rotated keys");
        assert_eq!(rotated_keys.public_key, rotated_public_key);

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn enabled_push_service_persists_subscriptions() {
        let db = test_db().await;
        let temp_root = std::env::temp_dir().join(format!("agenthub-push-{}", Uuid::new_v4()));
        let keys_path = temp_root.join("vapid.json");
        let config = test_push_config(&keys_path);
        let push = PushService::new(db.clone(), &config).expect("create push service");
        let subscription = test_push_subscription();

        push.save_subscription("user-1", subscription.clone())
            .await
            .expect("save subscription");

        let row =
            sqlx::query("SELECT user_id, endpoint, p256dh, auth FROM push_subscriptions LIMIT 1")
                .fetch_one(&db)
                .await
                .expect("fetch subscription");
        let user_id: String = row.get("user_id");
        let endpoint: String = row.get("endpoint");
        let p256dh: String = row.get("p256dh");
        let auth: String = row.get("auth");
        assert_eq!(user_id, "user-1");
        assert_eq!(endpoint, subscription.endpoint);
        assert_eq!(p256dh, subscription.keys.p256dh);
        assert_eq!(auth, subscription.keys.auth);

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn load_or_create_vapid_keys_rejects_missing_required_fields() {
        let temp_root = std::env::temp_dir().join(format!("agenthub-push-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).expect("create temp root");
        let keys_path = temp_root.join("vapid.json");
        std::fs::write(&keys_path, r#"{"public_key":"","private_key":""}"#)
            .expect("write invalid keys file");

        assert_eq!(
            load_or_create_vapid_keys(&keys_path)
                .expect_err("invalid keys file should fail")
                .to_string(),
            "vapid keys file missing required fields"
        );

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
