use super::*;

pub(super) fn directory() -> Option<std::path::PathBuf> {
    std::env::var_os("LOOP_NATIVE_BROWSER_DIR").map(std::path::PathBuf::from)
}

pub(super) struct Browser {
    directory: std::path::PathBuf,
    server: tokio::task::JoinHandle<()>,
}

impl Browser {
    pub(super) async fn start(
        fixture: &Fixture,
        local: &str,
        target: &agenthub_rara::RecoveryTarget,
    ) -> Option<Self> {
        let directory = directory()?;
        let web = std::path::PathBuf::from(std::env::var("LOOP_UI_WEB_DIR").unwrap());
        assert!(web.is_absolute() && web.join("index.html").is_file());
        let (user, token) = crate::api::team_tests::create_auth_token_with_role_and_user_id(
            &fixture.state,
            agenthub_auth_domain::UserRole::Root,
        )
        .await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let web = Arc::new(web);
        let app = Router::new()
            .nest("/api", crate::api::router(fixture.state.clone()))
            .nest("/sse", crate::sse::router(fixture.state.clone()))
            .fallback(move |uri| crate::web::dir_handler(web.clone(), uri));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        std::fs::write(
            directory.join("ready.json"),
            serde_json::to_vec_pretty(&json!({
                "origin":origin,"team_id":fixture.team_id,"local_session_id":local,"target":target,
                "auth":{"token":token,"userId":user,"username":"browser-owner","role":"root"}
            }))
            .unwrap(),
        )
        .unwrap();
        println!("Native recovery browser ready: {}", directory.display());
        Some(Self { directory, server })
    }

    pub(super) async fn wait_for_review(&self) {
        self.wait_for("reviewed").await;
    }

    async fn wait_for(&self, file: &str) {
        tokio::time::timeout(Duration::from_secs(300), async {
            while !self.directory.join(file).exists() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("browser completion signal");
    }

    pub(super) async fn finish(self) {
        std::fs::write(self.directory.join("completed"), "").unwrap();
        self.wait_for("stop").await;
        self.server.abort();
    }
}
