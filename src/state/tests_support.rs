use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::ffi::OsString;
use tokio::sync::Mutex;

pub(super) static ENV_LOCK: Mutex<()> = Mutex::const_new(());

pub(super) struct EnvGuard {
    key: &'static str,
    value: Option<OsString>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            unsafe {
                std::env::set_var(self.key, value);
            }
        } else {
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }
}

pub(super) fn set_env_var(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> EnvGuard {
    let guard = EnvGuard {
        key,
        value: std::env::var_os(key),
    };
    unsafe {
        std::env::set_var(key, value);
    }
    guard
}

pub(super) fn clear_env_var(key: &'static str) -> EnvGuard {
    let guard = EnvGuard {
        key,
        value: std::env::var_os(key),
    };
    unsafe {
        std::env::remove_var(key);
    }
    guard
}

pub(super) async fn test_db() -> sqlx::SqlitePool {
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
        CREATE TABLE users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            role TEXT NOT NULL,
            password_hash TEXT,
            created_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create users table");
    pool
}
