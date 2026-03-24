use base64::URL_SAFE_NO_PAD;
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
    subject: String,
    keys_path: PathBuf,
    keys: RwLock<VapidKeys>,
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
            subject,
            keys_path,
            keys: RwLock::new(keys),
        })
    }

    pub fn public_key(&self) -> String {
        self.keys
            .read()
            .expect("vapid keys lock poisoned")
            .public_key
            .clone()
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn keys_path(&self) -> &std::path::Path {
        &self.keys_path
    }

    pub fn rotate_keys(&self) -> anyhow::Result<String> {
        let keys = generate_vapid_keys()?;
        let payload = serde_json::to_string_pretty(&keys)?;
        std::fs::write(&self.keys_path, payload)?;
        let mut guard = self.keys.write().expect("vapid keys lock poisoned");
        *guard = keys;
        Ok(guard.public_key.clone())
    }

    pub async fn save_subscription(
        &self,
        user_id: &str,
        sub: PushSubscription,
    ) -> anyhow::Result<()> {
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
        for row in rows {
            let endpoint: String = row.get("endpoint");
            let p256dh: String = row.get("p256dh");
            let auth: String = row.get("auth");
            let subscription = SubscriptionInfo::new(endpoint, p256dh, auth);
            let private_key = {
                self.keys
                    .read()
                    .expect("vapid keys lock poisoned")
                    .private_key
                    .clone()
            };
            let mut sig_builder =
                VapidSignatureBuilder::from_base64(&private_key, &subscription)?;
            sig_builder.add_claim("sub", self.subject.as_str());
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
    let private_key = base64::encode_config(private_bytes, URL_SAFE_NO_PAD);
    let builder = VapidSignatureBuilder::from_base64_no_sub(&private_key)?;
    let public_key = base64::encode_config(builder.get_public_key(), URL_SAFE_NO_PAD);
    Ok(VapidKeysFile {
        public_key,
        private_key,
    })
}
