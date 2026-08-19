use agenthub_config::{
    AppConfig, InternalGrpcAuthConfig, InternalGrpcBootstrapConfig, InternalGrpcConfig,
    InternalGrpcSecurityConfig, PushConfig, ServerConfig, ServerRole, WebConfig,
};
use agenthub_db::AgentEventDbRouter;
use sqlx::Row;
use uuid::Uuid;

use super::AppState;
use super::gitignore::{
    DEFAULT_GIT_IGNORE_SUBPATH, GLOBAL_GITIGNORE_ENTRY, GLOBAL_GITIGNORE_FILENAME,
    append_gitignore_entry, resolve_global_gitignore_paths,
};
use super::tests_support::{ENV_LOCK, clear_env_var, set_env_var, test_db};

#[tokio::test]
async fn ensure_root_inserts_once() {
    let db = test_db().await;
    AppState::ensure_root(&db).await.expect("ensure root first");
    AppState::ensure_root(&db)
        .await
        .expect("ensure root second should be no-op");

    let row = sqlx::query("SELECT COUNT(*) AS cnt FROM users WHERE role = 'root'")
        .fetch_one(&db)
        .await
        .expect("count root users");
    let count: i64 = row.get("cnt");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn ensure_global_gitignore_contains_agenthubmemory_entry() {
    let _guard = ENV_LOCK.lock().await;
    let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_home).expect("create temp home");
    let _home_guard = set_env_var("HOME", &temp_home);
    let _xdg_guard = clear_env_var("XDG_CONFIG_HOME");

    AppState::ensure_global_gitignore_agenthubmemory().expect("ensure global gitignore");

    let gitignore_path = temp_home.join(GLOBAL_GITIGNORE_FILENAME);
    let content = std::fs::read_to_string(&gitignore_path).expect("read global gitignore");
    assert_eq!(content, ".agenthubmemory\n");

    let default_ignore_path = temp_home.join(".config").join(DEFAULT_GIT_IGNORE_SUBPATH);
    let default_content =
        std::fs::read_to_string(&default_ignore_path).expect("read default git ignore");
    assert_eq!(default_content, ".agenthubmemory\n");

    let _ = std::fs::remove_dir_all(&temp_home);
}

#[tokio::test]
async fn ensure_global_gitignore_keeps_agenthubmemory_entry_idempotent() {
    let _guard = ENV_LOCK.lock().await;
    let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_home).expect("create temp home");
    let gitignore_path = temp_home.join(GLOBAL_GITIGNORE_FILENAME);
    let default_ignore_path = temp_home.join(".config").join(DEFAULT_GIT_IGNORE_SUBPATH);
    std::fs::write(&gitignore_path, "*.log\n.agenthubmemory\n").expect("seed global gitignore");
    append_gitignore_entry(&default_ignore_path, GLOBAL_GITIGNORE_ENTRY)
        .expect("seed default gitignore");
    let _home_guard = set_env_var("HOME", &temp_home);
    let _xdg_guard = clear_env_var("XDG_CONFIG_HOME");

    AppState::ensure_global_gitignore_agenthubmemory().expect("ensure global gitignore");

    let content = std::fs::read_to_string(&gitignore_path).expect("read global gitignore");
    assert_eq!(content, "*.log\n.agenthubmemory\n");

    let default_content =
        std::fs::read_to_string(&default_ignore_path).expect("read default gitignore");
    assert_eq!(default_content, ".agenthubmemory\n");

    let _ = std::fs::remove_dir_all(&temp_home);
}

#[tokio::test]
async fn ensure_global_gitignore_prefers_xdg_config_home_when_present() {
    let _guard = ENV_LOCK.lock().await;
    let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
    let temp_xdg = std::env::temp_dir().join(format!("agenthub-xdg-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_home).expect("create temp home");
    std::fs::create_dir_all(&temp_xdg).expect("create temp xdg");
    let _home_guard = set_env_var("HOME", &temp_home);
    let _xdg_guard = set_env_var("XDG_CONFIG_HOME", &temp_xdg);

    let paths = resolve_global_gitignore_paths(&temp_home);
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], temp_home.join(GLOBAL_GITIGNORE_FILENAME));
    assert_eq!(paths[1], temp_xdg.join(DEFAULT_GIT_IGNORE_SUBPATH));

    AppState::ensure_global_gitignore_agenthubmemory().expect("ensure global gitignore");

    let xdg_content = std::fs::read_to_string(temp_xdg.join(DEFAULT_GIT_IGNORE_SUBPATH))
        .expect("read xdg git ignore");
    assert_eq!(xdg_content, ".agenthubmemory\n");

    let _ = std::fs::remove_dir_all(&temp_home);
    let _ = std::fs::remove_dir_all(&temp_xdg);
}

#[tokio::test]
async fn initialize_services_skips_internal_grpc_material_when_disabled() {
    let db = test_db().await;
    let cert_dir =
        std::env::temp_dir().join(format!("agenthub-state-internal-grpc-{}", Uuid::new_v4()));
    let keys_dir =
        std::env::temp_dir().join(format!("agenthub-state-push-keys-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&keys_dir).expect("create keys dir");
    let keys_path = keys_dir.join("vapid.json");
    let config = AppConfig {
        web: Some(WebConfig {
            rp_id: Some("localhost".to_string()),
            rp_origin: Some("http://localhost:8080".to_string()),
            rp_name: Some("AgentHub Test".to_string()),
            passkey_enabled: None,
        }),
        push: Some(PushConfig {
            subject: Some("mailto:test@example.com".to_string()),
            keys_path: Some(keys_path.to_string_lossy().to_string()),
        }),
        internal_grpc: Some(InternalGrpcConfig {
            enabled: Some(false),
            listen: None,
            security: Some(InternalGrpcSecurityConfig {
                mode: Some("mtls".to_string()),
                cert_dir: Some(cert_dir.to_string_lossy().to_string()),
            }),
            auth: None,
            bootstrap: None,
        }),
        ..Default::default()
    };

    let services = AppState::initialize_services(
        &config,
        db,
        AgentEventDbRouter::with_default_base_dir(),
        crate::message_body_store::MessageStores::default(),
    )
    .await;
    assert!(
        services.is_ok(),
        "initialize services should succeed when internal grpc is disabled"
    );
    assert!(
        !cert_dir.exists(),
        "disabled internal grpc should not create cert dir {}",
        cert_dir.display()
    );
    let _ = std::fs::remove_dir_all(&keys_dir);
}

#[tokio::test]
async fn setup_database_skips_root_user_for_node_role() {
    let _guard = ENV_LOCK.lock().await;
    let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_home).expect("create temp home");
    let _home_guard = set_env_var("HOME", &temp_home);

    let config = AppConfig {
        server: Some(ServerConfig {
            listen: None,
            role: Some(ServerRole::Node),
            node_id: Some("node-east".to_string()),
        }),
        ..Default::default()
    };

    let db = AppState::setup_database(&config)
        .await
        .expect("setup node database");

    let row = sqlx::query("SELECT COUNT(*) AS cnt FROM users WHERE role = 'root'")
        .fetch_one(&db)
        .await
        .expect("count root users");
    let count: i64 = row.get("cnt");
    assert_eq!(count, 0, "node startup should not create a root user");

    db.close().await;
    let _ = std::fs::remove_dir_all(&temp_home);
}

#[tokio::test]
async fn setup_database_creates_root_user_for_main_role() {
    let _guard = ENV_LOCK.lock().await;
    let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_home).expect("create temp home");
    let _home_guard = set_env_var("HOME", &temp_home);

    let config = AppConfig::default();

    let db = AppState::setup_database(&config)
        .await
        .expect("setup main database");

    let row = sqlx::query("SELECT COUNT(*) AS cnt FROM users WHERE role = 'root'")
        .fetch_one(&db)
        .await
        .expect("count root users");
    let count: i64 = row.get("cnt");
    assert_eq!(count, 1, "main startup should create a root user");

    db.close().await;
    let _ = std::fs::remove_dir_all(&temp_home);
}

#[tokio::test]
async fn initialize_services_disables_push_for_node_role() {
    let db = test_db().await;
    let temp_root = std::env::temp_dir().join(format!("agenthub-node-startup-{}", Uuid::new_v4()));
    let cert_dir = temp_root.join("internal-grpc");
    let keys_path = temp_root.join("push").join("vapid.json");
    let event_dir = temp_root.join("agent-events");
    let config = AppConfig {
        server: Some(ServerConfig {
            listen: None,
            role: Some(ServerRole::Node),
            node_id: Some("node-east".to_string()),
        }),
        push: Some(PushConfig {
            subject: Some("mailto:test@example.com".to_string()),
            keys_path: Some(keys_path.to_string_lossy().to_string()),
        }),
        internal_grpc: Some(InternalGrpcConfig {
            enabled: Some(true),
            listen: Some("127.0.0.1:50051".to_string()),
            security: Some(InternalGrpcSecurityConfig {
                mode: Some("disabled".to_string()),
                cert_dir: Some(cert_dir.to_string_lossy().to_string()),
            }),
            auth: None,
            bootstrap: None,
        }),
        ..Default::default()
    };

    let (_, _, push, _, _) = AppState::initialize_services(
        &config,
        db,
        AgentEventDbRouter::new(event_dir),
        crate::message_body_store::MessageStores::default(),
    )
    .await
    .expect("initialize node services");

    assert!(
        !push.is_enabled(),
        "node startup should keep push notifications disabled"
    );
    assert_eq!(
        push.public_key(),
        "",
        "node startup should not expose a VAPID public key"
    );
    assert!(
        !keys_path.exists(),
        "node startup should not create VAPID keys at {}",
        keys_path.display()
    );

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[tokio::test]
async fn initialize_services_enables_push_for_main_role() {
    let db = test_db().await;
    let temp_root = std::env::temp_dir().join(format!("agenthub-main-startup-{}", Uuid::new_v4()));
    let keys_path = temp_root.join("push").join("vapid.json");
    let event_dir = temp_root.join("agent-events");
    let config = AppConfig {
        server: Some(ServerConfig {
            listen: None,
            role: Some(ServerRole::Main),
            node_id: None,
        }),
        push: Some(PushConfig {
            subject: Some("mailto:test@example.com".to_string()),
            keys_path: Some(keys_path.to_string_lossy().to_string()),
        }),
        ..Default::default()
    };

    let (_, _, push, _, _) = AppState::initialize_services(
        &config,
        db,
        AgentEventDbRouter::new(event_dir),
        crate::message_body_store::MessageStores::default(),
    )
    .await
    .expect("initialize main services");

    assert!(
        push.is_enabled(),
        "main startup should enable push notifications"
    );
    assert!(
        keys_path.exists(),
        "main startup should materialize VAPID keys at {}",
        keys_path.display()
    );

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn build_agent_node_join_bootstrap_returns_disabled_when_internal_grpc_is_off() {
    let info = AppState::build_agent_node_join_bootstrap(&AppConfig::default())
        .expect("build disabled bootstrap info");

    assert!(!info.enabled);
    assert_eq!(info.bootstrap_token, None);
    assert_eq!(info.grpc_listen_addr, None);
    assert_eq!(info.security_mode, None);
    assert_eq!(info.cert_dir, None);
    assert_eq!(info.issuer, None);
    assert_eq!(info.audience, None);
}

#[test]
fn build_agent_node_join_bootstrap_uses_defaults_for_auth_fields() {
    let cert_dir =
        std::env::temp_dir().join(format!("agenthub-bootstrap-defaults-{}", Uuid::new_v4()));
    let config = AppConfig {
        internal_grpc: Some(InternalGrpcConfig {
            enabled: Some(true),
            listen: Some("0.0.0.0:50051".to_string()),
            security: Some(InternalGrpcSecurityConfig {
                mode: Some("tls".to_string()),
                cert_dir: Some(cert_dir.to_string_lossy().to_string()),
            }),
            auth: None,
            bootstrap: Some(InternalGrpcBootstrapConfig {
                token: Some("provided-token".to_string()),
            }),
        }),
        ..Default::default()
    };

    let info = AppState::build_agent_node_join_bootstrap(&config)
        .expect("build bootstrap info with default auth fields");

    assert!(info.enabled);
    assert_eq!(info.bootstrap_token.as_deref(), Some("provided-token"));
    assert_eq!(info.grpc_listen_addr.as_deref(), Some("0.0.0.0:50051"));
    assert_eq!(info.security_mode.as_deref(), Some("tls"));
    assert_eq!(
        info.cert_dir.as_deref(),
        Some(cert_dir.to_string_lossy().as_ref())
    );
    assert_eq!(info.issuer.as_deref(), Some("agenthub"));
    assert_eq!(info.audience.as_deref(), Some("agenthub-internal"));

    let _ = std::fs::remove_dir_all(&cert_dir);
}

#[test]
fn build_agent_node_join_bootstrap_respects_configured_auth_fields() {
    let cert_dir = std::env::temp_dir().join(format!("agenthub-bootstrap-auth-{}", Uuid::new_v4()));
    let config = AppConfig {
        internal_grpc: Some(InternalGrpcConfig {
            enabled: Some(true),
            listen: Some("127.0.0.1:50051".to_string()),
            security: Some(InternalGrpcSecurityConfig {
                mode: Some("disabled".to_string()),
                cert_dir: Some(cert_dir.to_string_lossy().to_string()),
            }),
            auth: Some(InternalGrpcAuthConfig {
                shared_secret: None,
                issuer: Some("custom-issuer".to_string()),
                audience: Some("custom-audience".to_string()),
            }),
            bootstrap: Some(InternalGrpcBootstrapConfig {
                token: Some("provided-token".to_string()),
            }),
        }),
        ..Default::default()
    };

    let info = AppState::build_agent_node_join_bootstrap(&config)
        .expect("build bootstrap info with explicit auth fields");

    assert_eq!(info.issuer.as_deref(), Some("custom-issuer"));
    assert_eq!(info.audience.as_deref(), Some("custom-audience"));

    let _ = std::fs::remove_dir_all(&cert_dir);
}
