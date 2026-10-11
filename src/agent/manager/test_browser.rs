//! Opt-in browser acceptance against real API/SSE handlers and an isolated native process.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use serde_json::{Value, json};

pub(super) struct BrowserServer {
    directory: PathBuf,
    server: tokio::task::JoinHandle<()>,
}

impl BrowserServer {
    pub(super) async fn start(
        state: &crate::state::AppState,
        directory: PathBuf,
        mut manifest: Value,
    ) -> Self {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let web = PathBuf::from(std::env::var("LOOP_UI_WEB_DIR").unwrap());
        assert!(web.is_absolute() && web.join("index.html").is_file());
        let (user, token) = crate::api::team_tests::create_auth_token_with_role_and_user_id(
            state,
            agenthub_auth_domain::UserRole::Root,
        )
        .await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let web = Arc::new(web);
        let app = Router::new()
            .nest("/api", crate::api::router(state.clone()))
            .nest("/sse", crate::sse::router(state.clone()))
            .fallback(move |uri| crate::web::dir_handler(web.clone(), uri));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        manifest["origin"] = json!(origin);
        manifest["auth"] =
            json!({"token":token,"userId":user,"username":"browser-owner","role":"root"});
        let pending = directory.join("ready.pending.json");
        let ready = directory.join("ready.json");
        assert!(!ready.exists(), "browser manifest already exists");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending)
            .unwrap();
        file.write_all(&serde_json::to_vec_pretty(&manifest).unwrap())
            .unwrap();
        file.sync_all().unwrap();
        std::fs::rename(pending, ready).unwrap();
        println!("Native recovery browser ready: {}", directory.display());
        Self { directory, server }
    }

    pub(super) fn signal(&self, file: &str) {
        std::fs::write(self.directory.join(file), "").unwrap();
    }

    pub(super) async fn wait_for(&self, file: &str) {
        tokio::time::timeout(Duration::from_secs(300), async {
            while !self.directory.join(file).exists() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("browser completion signal");
    }

    pub(super) async fn finish(self) {
        self.signal("completed");
        self.wait_for("stop").await;
    }
}

impl Drop for BrowserServer {
    fn drop(&mut self) {
        self.server.abort();
    }
}
