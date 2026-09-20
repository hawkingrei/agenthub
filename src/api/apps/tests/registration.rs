use super::*;

#[tokio::test]
async fn registration_requires_instance_configuration_and_keeps_connections_private() {
    let state = build_test_state().await;
    let (_, root) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let router = crate::api::router(state.clone());
    request(
        &router,
        Method::POST,
        "/apps",
        None,
        Some(registration(None, "one")),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    for role in [
        UserRole::Admin,
        UserRole::Operator,
        UserRole::Viewer,
        UserRole::Device,
    ] {
        let (_, token) = create_auth_token_with_role_and_user_id(&state, role).await;
        request(
            &router,
            Method::POST,
            "/apps",
            Some(&token),
            Some(registration(None, "one")),
            StatusCode::UNAUTHORIZED,
        )
        .await;
    }
    let (owner_id, owner) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let first = request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(registration(Some(&owner_id), "one")),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(first["owner_user_id"], owner_id);
    assert!(first.get("connection").is_none());
    let id = first["id"].as_str().unwrap();
    assert_eq!(
        request(
            &router,
            Method::GET,
            &format!("/apps/{id}"),
            Some(&owner),
            None,
            StatusCode::OK
        )
        .await,
        first
    );
    request(
        &router,
        Method::GET,
        &format!("/apps/{id}"),
        Some(&root),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(
        request(
            &router,
            Method::GET,
            "/apps",
            Some(&root),
            None,
            StatusCode::OK
        )
        .await,
        json!([])
    );
    let second = request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(registration(Some(&owner_id), "two")),
        StatusCode::CREATED,
    )
    .await;
    let page = request(
        &router,
        Method::GET,
        "/apps?limit=1",
        Some(&owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(page, json!([first]));
    let page = request(
        &router,
        Method::GET,
        &format!("/apps?after={id}&limit=1"),
        Some(&owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(page, json!([second]));
    request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(registration(Some(&owner_id), "one")),
        StatusCode::CONFLICT,
    )
    .await;
    let version = request(
        &router,
        Method::GET,
        &format!("/apps/{id}/versions/1"),
        Some(&owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(version["manifest"]["tools"][0]["name"], "lookup");
    request(
        &router,
        Method::GET,
        &format!("/apps/{id}/versions/1"),
        Some(&root),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::GET,
        &format!("/apps/{id}/versions/0"),
        Some(&owner),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
}

#[tokio::test]
async fn manifest_publication_requires_owner_and_preserves_old_versions() {
    let state = build_test_state().await;
    let (owner_id, owner) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let (_, other) = create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let router = crate::api::router(state.clone());
    let app = request(
        &router,
        Method::POST,
        "/apps",
        Some(&owner),
        Some(registration(None, "versioned")),
        StatusCode::CREATED,
    )
    .await;
    let id = app["id"].as_str().unwrap();
    let path = format!("/apps/{id}/versions");
    let mut changed = manifest();
    changed["tools"][0]["input_schema"]["properties"] = json!({"key": {"type": "string"}});
    request(
        &router,
        Method::POST,
        &path,
        Some(&other),
        Some(json!({"expected_revision":1,"manifest":changed})),
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::POST,
        &path,
        Some(&owner),
        Some(json!({"expected_revision":0,"manifest":changed})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let published = request(
        &router,
        Method::POST,
        &path,
        Some(&owner),
        Some(json!({"expected_revision":1,"manifest":changed})),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(published["version"], 2);
    request(
        &router,
        Method::POST,
        &path,
        Some(&owner),
        Some(json!({"expected_revision":1,"manifest":manifest()})),
        StatusCode::CONFLICT,
    )
    .await;
    let old = request(
        &router,
        Method::GET,
        &format!("{path}/1"),
        Some(&owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert!(
        old["manifest"]["tools"][0]["input_schema"]
            .get("properties")
            .is_none()
    );
    sqlx::query("UPDATE users SET role = 'viewer' WHERE id = ?")
        .bind(&owner_id)
        .execute(&state.db)
        .await
        .unwrap();
    request(
        &router,
        Method::POST,
        &path,
        Some(&owner),
        Some(json!({"expected_revision":2,"manifest":changed})),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    request(
        &router,
        Method::GET,
        &format!("/apps/{id}"),
        Some(&owner),
        None,
        StatusCode::OK,
    )
    .await;
    sqlx::query("UPDATE users SET role = 'root' WHERE id = ?")
        .bind(&owner_id)
        .execute(&state.db)
        .await
        .unwrap();
    request(
        &router,
        Method::POST,
        &format!("/apps/{id}/revoke"),
        Some(&other),
        Some(json!({"expected_revision":2})),
        StatusCode::NOT_FOUND,
    )
    .await;
    let revoked = request(
        &router,
        Method::POST,
        &format!("/apps/{id}/revoke"),
        Some(&owner),
        Some(json!({"expected_revision":2})),
        StatusCode::OK,
    )
    .await;
    assert!(revoked["revoked_at"].is_number());
    request(
        &router,
        Method::POST,
        &path,
        Some(&owner),
        Some(json!({"expected_revision":3,"manifest":changed})),
        StatusCode::CONFLICT,
    )
    .await;
}
