use super::*;

pub(super) struct Browser {
    directory: std::path::PathBuf,
    server: tokio::task::JoinHandle<()>,
}

impl Browser {
    pub(super) async fn start(fixture: &Fixture, database: &std::path::Path) -> Option<Self> {
        let directory = std::env::var_os("LOOP_REAL_BROWSER_DIR").map(std::path::PathBuf::from)?;
        std::fs::create_dir_all(&directory).unwrap();
        assert!(
            !directory.join("ready.json").exists(),
            "use a fresh browser directory"
        );
        let web = std::path::PathBuf::from(std::env::var("LOOP_UI_WEB_DIR").unwrap());
        assert!(web.is_absolute() && web.join("index.html").is_file());
        let now = Utc::now().timestamp();
        sqlx::query("INSERT INTO team_members(team_id, user_id, role, created_at, updated_at) VALUES (?, 'app-owner', 'owner', ?, ?)")
            .bind(&fixture.team_id).bind(now).bind(now).execute(&fixture.state.db).await.unwrap();
        let token = fixture
            .state
            .auth
            .create_session("app-owner")
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let web = Arc::new(web);
        let app = axum::Router::new()
            .nest("/api", crate::api::router(fixture.state.clone()))
            .nest("/sse", crate::sse::router(fixture.state.clone()))
            .fallback(move |uri| crate::web::dir_handler(web.clone(), uri));
        std::fs::write(fixture.directory.join("browser-hold"), "").unwrap();
        std::fs::write(directory.join("ready.json"), serde_json::to_vec_pretty(&json!({
            "origin":origin,"team_id":fixture.team_id,"database":database,"provider_directory":fixture.directory,
            "auth":{"token":token,"userId":"app-owner","username":"app-owner","role":"admin"}
        })).unwrap()).unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        println!(
            "Real runtime browser fixture ready: {}",
            directory.display()
        );
        Some(Self { directory, server })
    }

    pub(super) async fn finish(self) {
        std::fs::write(self.directory.join("completed"), "").unwrap();
        tokio::time::timeout(Duration::from_secs(300), async {
            while !self.directory.join("stop").exists() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("browser fixture stop signal");
        self.server.abort();
    }
}
