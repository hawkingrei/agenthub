//! Opt-in browser harness using production schema, real HTTP routes, and the bounded fake provider.

use super::*;

#[tokio::test]
#[ignore = "manual Chrome DevTools browser fixture; requires LOOP_UI_FIXTURE_DIR and LOOP_UI_WEB_DIR"]
async fn loop_browser_fixture() {
    let directory = std::path::PathBuf::from(std::env::var("LOOP_UI_FIXTURE_DIR").unwrap());
    let web = std::path::PathBuf::from(std::env::var("LOOP_UI_WEB_DIR").unwrap());
    assert!(directory.is_absolute() && web.join("index.html").is_file());
    std::fs::create_dir_all(&directory).unwrap();
    let database = directory.join("control.sqlite");
    assert!(!database.exists(), "use a fresh isolated fixture directory");
    let pool = agenthub_db::init_db_at_path(&database).await.unwrap();
    pool.close().await;
    let state = crate::api::team_tests::reopen_test_state_with_db_path(&database).await;
    let fixture = Fixture::with_state(state, "finish", None).await;
    sqlx::query("UPDATE team_definitions SET name = 'Durable work demo', description = 'Configure offline, run bounded work, and inspect retained history.' WHERE id = ?")
        .bind(&fixture.team_id).execute(&fixture.state.db).await.unwrap();
    let now = Utc::now().timestamp();
    LoopStore::new(fixture.state.db.clone())
        .configure(
            LoopPolicyUpdate {
                actor_id: "worker",
                team_id: &fixture.team_id,
                expected_revision: 2,
                state: LoopPolicyState::Disabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            now,
        )
        .await
        .unwrap();
    let user_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO users(id, username, display_name, role, password_hash, created_at) VALUES (?, 'browser-owner', 'Browser Owner', 'admin', NULL, ?)")
        .bind(&user_id).bind(now).execute(&fixture.state.db).await.unwrap();
    sqlx::query("INSERT INTO team_members(team_id, user_id, role, created_at, updated_at) VALUES (?, ?, 'owner', ?, ?)")
        .bind(&fixture.team_id).bind(&user_id).bind(now).bind(now).execute(&fixture.state.db).await.unwrap();
    let token = fixture.state.auth.create_session(&user_id).await.unwrap();
    fixture
        .state
        .agents
        .spawn_loop_worker(fixture.state.teams.clone())
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let web = Arc::new(web);
    let app = axum::Router::new()
        .route("/health", axum::routing::get(crate::api::health))
        .nest("/api", crate::api::router(fixture.state.clone()))
        .nest("/sse", crate::sse::router(fixture.state.clone()))
        .fallback(move |uri| crate::web::dir_handler(web.clone(), uri));
    let manifest = directory.join("ready.json");
    std::fs::write(&manifest, serde_json::to_vec_pretty(&serde_json::json!({
        "origin": origin, "team_id": fixture.team_id, "database": database,
        "provider_directory": fixture.directory,
        "auth": {"token": token, "userId": user_id, "username": "browser-owner", "role": "admin"},
    })).unwrap()).unwrap();
    println!("Browser fixture ready: {}", manifest.display());
    // Browser closure must not cancel durable execution. Only the operator's stop file ends the fixture.
    let stop = directory.join("stop");
    tokio::select! {
        result = axum::serve(listener, app) => result.unwrap(),
        _ = async {
            while !stop.exists() {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        } => {}
    }
    fixture.close().await;
}
