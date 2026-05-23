use std::collections::BTreeSet;
use std::time::Duration;

use base64::Engine;
use chrono::Utc;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use url::Url;
use uuid::Uuid;

pub const SLOCK_CONNECTOR_ID: &str = "slock";
pub const DEFAULT_SLOCK_LINKER_ID: &str = "slock-primary";
const SLOCK_PROVIDER: &str = "slock";
const DEFAULT_SLOCK_DISPLAY_NAME: &str = "Slock";
const DEFAULT_SLOCK_API_ORIGIN: &str = "https://api.slock.ai";
const SLOCK_TOKEN_PATH: &str = "/api/oauth/token";
const SLOCK_USERINFO_PATH: &str = "/api/oauth/userinfo";
const LINK_ATTEMPT_TTL_SECONDS: i64 = 10 * 60;
const HTTP_TIMEOUT_SECONDS: u64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlockConfig {
    pub api_origin: String,
    pub client_id: String,
    pub return_url: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SlockConfigInput {
    pub api_origin: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub return_url: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppLinkerPrincipal {
    pub subject: String,
    pub principal_type: String,
    pub display_name: String,
    pub handle: Option<String>,
    pub avatar_url: Option<String>,
    pub server_id: Option<String>,
    pub server_slug: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppLinkerRecord {
    pub linker_id: String,
    pub connector_id: String,
    pub display_name: String,
    pub status: String,
    pub api_origin: String,
    pub client_id: String,
    pub return_url: String,
    pub scopes: Vec<String>,
    pub client_secret_configured: bool,
    pub token_configured: bool,
    pub token_type: Option<String>,
    pub granted_scopes: Vec<String>,
    pub expires_at: Option<i64>,
    pub principal: Option<AppLinkerPrincipal>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SlockLinkAttempt {
    pub linker_id: String,
    pub state: String,
    pub expires_at: i64,
    pub return_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlockExchangeCode {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Clone)]
pub struct AppLinkerService {
    db: SqlitePool,
    http: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct SlockTokenResponse {
    access_token: String,
    token_type: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SlockUserinfoResponse {
    sub: String,
    #[serde(rename = "type")]
    principal_type: String,
    scope: Option<String>,
    client_id: Option<String>,
    client_name: Option<String>,
    server_id: Option<String>,
    server_slug: Option<String>,
    preferred_username: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
    description: Option<String>,
}

impl AppLinkerService {
    pub fn new(db: SqlitePool, http: reqwest::Client) -> Self {
        Self { db, http }
    }

    pub fn default_http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    }

    pub async fn list_linkers(&self) -> anyhow::Result<Vec<AppLinkerRecord>> {
        self.ensure_schema().await?;
        let rows = sqlx::query(
            r#"
            SELECT
                l.id,
                l.connector_id,
                l.display_name,
                l.status,
                l.config_json,
                l.updated_at,
                s.client_secret,
                s.access_token,
                s.token_type,
                s.scope,
                s.expires_at,
                p.subject,
                p.principal_type,
                p.display_name AS principal_display_name,
                p.handle,
                p.avatar_url,
                p.server_id,
                p.server_slug,
                p.updated_at AS principal_updated_at
            FROM app_linkers l
            LEFT JOIN app_linker_secrets s ON s.linker_id = l.id
            LEFT JOIN app_linker_principals p ON p.linker_id = l.id
            ORDER BY l.updated_at DESC, l.id ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        rows.into_iter().map(record_from_row).collect()
    }

    pub async fn get_slock_linker(&self) -> anyhow::Result<Option<AppLinkerRecord>> {
        self.ensure_schema().await?;
        let row = sqlx::query(
            r#"
            SELECT
                l.id,
                l.connector_id,
                l.display_name,
                l.status,
                l.config_json,
                l.updated_at,
                s.client_secret,
                s.access_token,
                s.token_type,
                s.scope,
                s.expires_at,
                p.subject,
                p.principal_type,
                p.display_name AS principal_display_name,
                p.handle,
                p.avatar_url,
                p.server_id,
                p.server_slug,
                p.updated_at AS principal_updated_at
            FROM app_linkers l
            LEFT JOIN app_linker_secrets s ON s.linker_id = l.id
            LEFT JOIN app_linker_principals p ON p.linker_id = l.id
            WHERE l.id = ?1
            "#,
        )
        .bind(DEFAULT_SLOCK_LINKER_ID)
        .fetch_optional(&self.db)
        .await?;
        row.map(record_from_row).transpose()
    }

    pub async fn upsert_slock_config(
        &self,
        user_id: &str,
        input: SlockConfigInput,
    ) -> anyhow::Result<AppLinkerRecord> {
        self.ensure_schema().await?;
        let config = normalize_slock_config(&input)?;
        let client_secret = input
            .client_secret
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let existing_secret: Option<String> =
            sqlx::query_scalar("SELECT client_secret FROM app_linker_secrets WHERE linker_id = ?1")
                .bind(DEFAULT_SLOCK_LINKER_ID)
                .fetch_optional(&self.db)
                .await?;
        if client_secret.is_none() && existing_secret.is_none() {
            anyhow::bail!("Slock client_secret is required before linking");
        }

        let now = Utc::now().timestamp();
        let config_json = serde_json::to_string(&config)?;
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO app_linkers (
                id, connector_id, display_name, provider, status,
                config_json, created_by_user_id, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, 'configured', ?5, ?6, ?7, ?7)
            ON CONFLICT(id) DO UPDATE SET
                connector_id = excluded.connector_id,
                display_name = excluded.display_name,
                provider = excluded.provider,
                status = 'configured',
                config_json = excluded.config_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(DEFAULT_SLOCK_LINKER_ID)
        .bind(SLOCK_CONNECTOR_ID)
        .bind(DEFAULT_SLOCK_DISPLAY_NAME)
        .bind(SLOCK_PROVIDER)
        .bind(config_json)
        .bind(user_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO app_linker_secrets (
                linker_id, client_secret, access_token, token_type, scope, expires_at, updated_at
            )
            VALUES (?1, ?2, NULL, NULL, NULL, NULL, ?3)
            ON CONFLICT(linker_id) DO UPDATE SET
                client_secret = COALESCE(excluded.client_secret, app_linker_secrets.client_secret),
                access_token = NULL,
                token_type = NULL,
                scope = NULL,
                expires_at = NULL,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(DEFAULT_SLOCK_LINKER_ID)
        .bind(client_secret)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM app_linker_principals WHERE linker_id = ?1")
            .bind(DEFAULT_SLOCK_LINKER_ID)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        self.get_slock_linker()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Slock linker was not created"))
    }

    pub async fn create_slock_link_attempt(
        &self,
        user_id: &str,
    ) -> anyhow::Result<SlockLinkAttempt> {
        self.ensure_schema().await?;
        let record = self
            .get_slock_linker()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Slock linker is not configured"))?;
        if !record.client_secret_configured {
            anyhow::bail!("Slock client_secret is not configured");
        }

        let now = Utc::now().timestamp();
        let expires_at = now + LINK_ATTEMPT_TTL_SECONDS;
        let state = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO app_linker_attempts (state, linker_id, created_by_user_id, expires_at, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(&state)
        .bind(DEFAULT_SLOCK_LINKER_ID)
        .bind(user_id)
        .bind(expires_at)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(SlockLinkAttempt {
            linker_id: DEFAULT_SLOCK_LINKER_ID.to_string(),
            state,
            expires_at,
            return_url: record.return_url,
        })
    }

    pub async fn exchange_slock_code(
        &self,
        user_id: Option<&str>,
        code: SlockExchangeCode,
    ) -> anyhow::Result<AppLinkerRecord> {
        self.ensure_schema().await?;
        let linker_id = match code.state.as_deref() {
            Some(state) => self.consume_link_attempt(state, user_id).await?,
            None => {
                anyhow::ensure!(
                    user_id.is_some(),
                    "state is required for unauthenticated Slock callbacks"
                );
                DEFAULT_SLOCK_LINKER_ID.to_string()
            }
        };
        anyhow::ensure!(
            linker_id == DEFAULT_SLOCK_LINKER_ID,
            "unsupported Slock linker id: {linker_id}"
        );
        let (config, client_secret) = self.load_slock_config_and_secret(&linker_id).await?;
        let token = self
            .exchange_token(&config, &client_secret, &code.code)
            .await?;
        let userinfo = self.load_userinfo(&config, &token.access_token).await?;
        self.store_exchange_result(&linker_id, &config, token, userinfo)
            .await?;
        self.get_slock_linker()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Slock linker was not found after exchange"))
    }

    pub async fn list_slock_channels(&self) -> anyhow::Result<Value> {
        self.ensure_schema().await?;
        let record = self
            .get_slock_linker()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Slock linker is not configured"))?;
        anyhow::ensure!(record.token_configured, "Slock linker is not connected");
        anyhow::bail!(
            "Slock channel resource API is not configured yet; complete the Slock resource endpoint contract before enabling channel reads"
        )
    }

    pub async fn list_slock_channel_messages(&self, channel_id: &str) -> anyhow::Result<Value> {
        self.ensure_schema().await?;
        anyhow::ensure!(
            !channel_id.trim().is_empty(),
            "channel_id must be a non-empty string"
        );
        let record = self
            .get_slock_linker()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Slock linker is not configured"))?;
        anyhow::ensure!(record.token_configured, "Slock linker is not connected");
        anyhow::bail!(
            "Slock channel message resource API is not configured yet; complete the Slock resource endpoint contract before enabling message reads"
        )
    }

    async fn ensure_schema(&self) -> anyhow::Result<()> {
        agenthub_db::ensure_app_linker_schema(&self.db).await
    }

    async fn consume_link_attempt(
        &self,
        state: &str,
        user_id: Option<&str>,
    ) -> anyhow::Result<String> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT linker_id, created_by_user_id, expires_at
            FROM app_linker_attempts
            WHERE state = ?1
            "#,
        )
        .bind(state)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("invalid or expired Slock link state"))?;
        let linker_id: String = row.get("linker_id");
        let created_by_user_id: String = row.get("created_by_user_id");
        let expires_at: i64 = row.get("expires_at");
        sqlx::query("DELETE FROM app_linker_attempts WHERE state = ?1")
            .bind(state)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        anyhow::ensure!(expires_at >= now, "Slock link state has expired");
        if let Some(user_id) = user_id {
            anyhow::ensure!(
                user_id == created_by_user_id,
                "Slock link state belongs to a different user"
            );
        }
        Ok(linker_id)
    }

    async fn load_slock_config_and_secret(
        &self,
        linker_id: &str,
    ) -> anyhow::Result<(SlockConfig, String)> {
        let row = sqlx::query(
            r#"
            SELECT l.config_json, s.client_secret
            FROM app_linkers l
            LEFT JOIN app_linker_secrets s ON s.linker_id = l.id
            WHERE l.id = ?1
            "#,
        )
        .bind(linker_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Slock linker is not configured"))?;
        let config_json: String = row.get("config_json");
        let client_secret: Option<String> = row.get("client_secret");
        let client_secret = client_secret
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Slock client_secret is not configured"))?;
        let config = serde_json::from_str::<SlockConfig>(&config_json)?;
        Ok((config, client_secret))
    }

    async fn exchange_token(
        &self,
        config: &SlockConfig,
        client_secret: &str,
        code: &str,
    ) -> anyhow::Result<SlockTokenResponse> {
        let token_url = format!("{}{}", config.api_origin, SLOCK_TOKEN_PATH);
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{client_secret}", config.client_id));
        let response = self
            .http
            .post(token_url)
            .header(header::AUTHORIZATION, format!("Basic {encoded}"))
            .json(&serde_json::json!({
                "grant_type": "authorization_code",
                "code": code,
            }))
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Slock token exchange failed ({status}): {body}");
        }
        let token = response.json::<SlockTokenResponse>().await?;
        anyhow::ensure!(
            !token.access_token.trim().is_empty(),
            "Slock token response omitted access_token"
        );
        Ok(token)
    }

    async fn load_userinfo(
        &self,
        config: &SlockConfig,
        access_token: &str,
    ) -> anyhow::Result<SlockUserinfoResponse> {
        let userinfo_url = format!("{}{}", config.api_origin, SLOCK_USERINFO_PATH);
        let response = self
            .http
            .get(userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Slock userinfo failed ({status}): {body}");
        }
        let userinfo = response.json::<SlockUserinfoResponse>().await?;
        anyhow::ensure!(
            !userinfo.sub.trim().is_empty(),
            "Slock userinfo omitted sub"
        );
        anyhow::ensure!(
            userinfo.principal_type == "human" || userinfo.principal_type == "agent",
            "unsupported Slock principal type '{}'",
            userinfo.principal_type
        );
        if let Some(client_id) = userinfo.client_id.as_deref() {
            anyhow::ensure!(
                client_id == config.client_id,
                "Slock userinfo client_id '{}' does not match configured client_id '{}'",
                client_id,
                config.client_id
            );
        }
        Ok(userinfo)
    }

    async fn store_exchange_result(
        &self,
        linker_id: &str,
        config: &SlockConfig,
        token: SlockTokenResponse,
        userinfo: SlockUserinfoResponse,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let expires_at = token
            .expires_in
            .map(|ttl| now.saturating_add(ttl.max(0)))
            .unwrap_or(now + 3600);
        let token_type = token
            .token_type
            .unwrap_or_else(|| "Bearer".to_string())
            .trim()
            .to_string();
        let granted_scope = token
            .scope
            .or_else(|| userinfo.scope.clone())
            .unwrap_or_else(|| config.scopes.join(" "));
        let display_name = userinfo
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or(userinfo
                .preferred_username
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()))
            .unwrap_or(userinfo.sub.as_str())
            .to_string();
        let handle = userinfo
            .preferred_username
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let avatar_url = userinfo
            .avatar_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let server_slug = userinfo
            .server_slug
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let server_id = userinfo
            .server_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let raw_userinfo_json = serde_json::to_string(&userinfo)?;
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            UPDATE app_linkers
            SET status = 'connected', updated_at = ?2
            WHERE id = ?1
            "#,
        )
        .bind(linker_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE app_linker_secrets
            SET access_token = ?2,
                token_type = ?3,
                scope = ?4,
                expires_at = ?5,
                updated_at = ?6
            WHERE linker_id = ?1
            "#,
        )
        .bind(linker_id)
        .bind(&token.access_token)
        .bind(token_type)
        .bind(granted_scope)
        .bind(expires_at)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO app_linker_principals (
                linker_id, subject, principal_type, display_name, handle,
                avatar_url, server_id, server_slug, raw_userinfo_json, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(linker_id) DO UPDATE SET
                subject = excluded.subject,
                principal_type = excluded.principal_type,
                display_name = excluded.display_name,
                handle = excluded.handle,
                avatar_url = excluded.avatar_url,
                server_id = excluded.server_id,
                server_slug = excluded.server_slug,
                raw_userinfo_json = excluded.raw_userinfo_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(linker_id)
        .bind(userinfo.sub)
        .bind(userinfo.principal_type)
        .bind(display_name)
        .bind(handle)
        .bind(avatar_url)
        .bind(server_id)
        .bind(server_slug)
        .bind(raw_userinfo_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

pub fn parse_slock_exchange_code(
    code: Option<&str>,
    callback_url: Option<&str>,
    state: Option<&str>,
) -> anyhow::Result<SlockExchangeCode> {
    let mut parsed_code = code
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut parsed_state = state
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if let Some(callback_url) = callback_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let url = Url::parse(callback_url)?;
        let mut query_code = None;
        let mut query_state = None;
        for (key, value) in url.query_pairs() {
            if key == "code" {
                query_code = Some(value.trim().to_string());
            } else if key == "state" {
                query_state = Some(value.trim().to_string());
            }
        }
        if parsed_code.is_none() {
            parsed_code = query_code.filter(|value| !value.is_empty());
        }
        if parsed_state.is_none() {
            parsed_state = query_state.filter(|value| !value.is_empty());
        }
    }

    let code = parsed_code.ok_or_else(|| anyhow::anyhow!("Slock callback code is required"))?;
    Ok(SlockExchangeCode {
        code,
        state: parsed_state,
    })
}

fn normalize_slock_config(input: &SlockConfigInput) -> anyhow::Result<SlockConfig> {
    let api_origin = normalize_origin(&input.api_origin, "api_origin")?;
    let client_id = required_trimmed(&input.client_id, "client_id")?;
    let return_url = required_trimmed(&input.return_url, "return_url")?;
    let return_url = Url::parse(&return_url)
        .map_err(|err| anyhow::anyhow!("invalid return_url: {err}"))?
        .to_string();
    let scopes = normalize_scopes(&input.scopes);
    Ok(SlockConfig {
        api_origin,
        client_id,
        return_url,
        scopes,
    })
}

fn normalize_origin(value: &str, field: &str) -> anyhow::Result<String> {
    let value = required_trimmed(value, field)?;
    let url = Url::parse(&value).map_err(|err| anyhow::anyhow!("invalid {field}: {err}"))?;
    anyhow::ensure!(
        url.scheme() == "http" || url.scheme() == "https",
        "{field} must use http or https"
    );
    anyhow::ensure!(url.host_str().is_some(), "{field} must include a hostname");
    anyhow::ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "{field} must not include query or fragment"
    );
    let mut origin = format!(
        "{}://{}",
        url.scheme(),
        url.host_str().expect("host checked above")
    );
    if let Some(port) = url.port() {
        origin.push(':');
        origin.push_str(&port.to_string());
    }
    Ok(origin)
}

fn required_trimmed(value: &str, field: &str) -> anyhow::Result<String> {
    let value = value.trim();
    anyhow::ensure!(!value.is_empty(), "{field} is required");
    Ok(value.to_string())
}

fn normalize_scopes(scopes: &[String]) -> Vec<String> {
    let mut values = scopes
        .iter()
        .flat_map(|scope| scope.split_whitespace())
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if values.is_empty() {
        values.extend(["identity", "openid", "profile"].map(str::to_string));
    }
    values.into_iter().collect()
}

fn split_scopes(scope: Option<String>) -> Vec<String> {
    scope
        .unwrap_or_default()
        .split_whitespace()
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(str::to_string)
        .collect()
}

fn record_from_row(row: sqlx::sqlite::SqliteRow) -> anyhow::Result<AppLinkerRecord> {
    let config_json: String = row.get("config_json");
    let config: SlockConfig = serde_json::from_str(&config_json)?;
    let client_secret: Option<String> = row.get("client_secret");
    let access_token: Option<String> = row.get("access_token");
    let subject: Option<String> = row.get("subject");
    let principal = subject.map(|subject| AppLinkerPrincipal {
        subject,
        principal_type: row.get("principal_type"),
        display_name: row.get("principal_display_name"),
        handle: row.get("handle"),
        avatar_url: row.get("avatar_url"),
        server_id: row.get("server_id"),
        server_slug: row.get("server_slug"),
        updated_at: row.get("principal_updated_at"),
    });

    Ok(AppLinkerRecord {
        linker_id: row.get("id"),
        connector_id: row.get("connector_id"),
        display_name: row.get("display_name"),
        status: row.get("status"),
        api_origin: config.api_origin,
        client_id: config.client_id,
        return_url: config.return_url,
        scopes: config.scopes,
        client_secret_configured: client_secret
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        token_configured: access_token
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        token_type: row.get("token_type"),
        granted_scopes: split_scopes(row.get("scope")),
        expires_at: row.get("expires_at"),
        principal,
        updated_at: row.get("updated_at"),
    })
}

impl Default for SlockConfigInput {
    fn default() -> Self {
        Self {
            api_origin: DEFAULT_SLOCK_API_ORIGIN.to_string(),
            client_id: String::new(),
            client_secret: None,
            return_url: String::new(),
            scopes: vec![
                "identity".to_string(),
                "openid".to_string(),
                "profile".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_slock_exchange_code_prefers_explicit_state_and_extracts_url_code() {
        let parsed = parse_slock_exchange_code(
            None,
            Some("https://agenthub.example/api/linkers/slock/callback?code=abc&state=from-url"),
            Some("explicit-state"),
        )
        .expect("parse exchange");
        assert_eq!(parsed.code, "abc");
        assert_eq!(parsed.state.as_deref(), Some("explicit-state"));
    }

    #[test]
    fn normalize_scopes_defaults_and_deduplicates() {
        assert_eq!(
            normalize_scopes(&["profile identity".to_string(), "profile".to_string()]),
            vec!["identity".to_string(), "profile".to_string()]
        );
        assert_eq!(
            normalize_scopes(&[]),
            vec![
                "identity".to_string(),
                "openid".to_string(),
                "profile".to_string()
            ]
        );
    }
}
