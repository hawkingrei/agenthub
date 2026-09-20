use super::*;

#[tokio::test]
async fn event_keys_require_instance_authority_and_expose_only_safe_owner_state() {
    let state = build_test_state().await;
    let (_, root) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let (owner_id, owner) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let (_, other) = create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let router = crate::api::router(state.clone());
    let app = request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(registration(Some(&owner_id), "events")),
        StatusCode::CREATED,
    )
    .await;
    let id = app["id"].as_str().unwrap();
    let path = format!("/apps/{id}/event-key");
    let config = json!({"expected_version":0,"credential_env":"PRIVATE_EVENT_KEY"});
    request(
        &router,
        Method::PUT,
        &path,
        None,
        Some(config.clone()),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &path,
        Some(&owner),
        Some(config.clone()),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &path,
        Some(&root),
        Some(json!({"expected_version":0,"credential_env":"HOME"})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let key = request(
        &router,
        Method::PUT,
        &path,
        Some(&root),
        Some(config.clone()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(key["version"], 1);
    assert!(!key.to_string().contains("PRIVATE_EVENT_KEY") && key.get("credential_env").is_none());
    request(
        &router,
        Method::PUT,
        &path,
        Some(&root),
        Some(config),
        StatusCode::CONFLICT,
    )
    .await;
    assert_eq!(
        request(
            &router,
            Method::GET,
            &path,
            Some(&owner),
            None,
            StatusCode::OK
        )
        .await,
        key
    );
    for token in [&root, &other] {
        request(
            &router,
            Method::GET,
            &path,
            Some(token),
            None,
            StatusCode::NOT_FOUND,
        )
        .await;
        request(
            &router,
            Method::POST,
            &format!("{path}/revoke"),
            Some(token),
            Some(json!({"expected_version":1})),
            StatusCode::NOT_FOUND,
        )
        .await;
    }
    let revoked = request(
        &router,
        Method::POST,
        &format!("{path}/revoke"),
        Some(&owner),
        Some(json!({"expected_version":1})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(revoked["version"], 2);
    assert!(revoked["revoked_at"].is_number());
    assert!(
        agenthub_db::app_registry::AppRegistry::new(state.db)
            .event_signing_key(id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn event_routes_need_separate_team_owner_approval_of_declared_classes() {
    let state = build_test_state().await;
    let (_, root) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let (owner_id, owner) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let (_, other) = create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let router = crate::api::router(state.clone());
    let mut input = registration(Some(&owner_id), "routed-events");
    input["manifest"]["events"] = json!([
        {"name":"changed","required_scopes":["read"]},
        {"name":"written","required_scopes":["write"]}
    ]);
    let app = request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(input),
        StatusCode::CREATED,
    )
    .await;
    let id = app["id"].as_str().unwrap();
    let team_id = team(&state, Some(&owner_id), "planner").await;
    request(
        &router,
        Method::PUT,
        &format!("/teams/{team_id}/apps/{id}"),
        Some(&owner),
        Some(json!({"expected_revision":0,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    let binding = format!("/teams/{team_id}/members/planner/apps/{id}");
    request(
        &router,
        Method::PUT,
        &binding,
        Some(&owner),
        Some(json!({"expected_revision":0,"version":1,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    let path = format!("{binding}/events");
    request(
        &router,
        Method::GET,
        &path,
        Some(&owner),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    let config = json!({"expected_revision":0,"classes":["changed"]});
    request(
        &router,
        Method::PUT,
        &path,
        None,
        Some(config.clone()),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &path,
        Some(&other),
        Some(config.clone()),
        StatusCode::NOT_FOUND,
    )
    .await;
    for classes in [json!([]), json!(["bad class"])] {
        request(
            &router,
            Method::PUT,
            &path,
            Some(&owner),
            Some(json!({"expected_revision":0,"classes":classes})),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    for class in ["written", "unknown"] {
        request(
            &router,
            Method::PUT,
            &path,
            Some(&owner),
            Some(json!({"expected_revision":0,"classes":[class]})),
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    let route = request(
        &router,
        Method::PUT,
        &path,
        Some(&owner),
        Some(config.clone()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(route["classes"], json!(["changed"]));
    assert_eq!(route["version"], 1);
    request(
        &router,
        Method::PUT,
        &path,
        Some(&owner),
        Some(config),
        StatusCode::CONFLICT,
    )
    .await;
    assert_eq!(
        request(
            &router,
            Method::GET,
            &path,
            Some(&owner),
            None,
            StatusCode::OK
        )
        .await,
        route
    );
    request(
        &router,
        Method::GET,
        &path,
        Some(&other),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("{path}/revoke"),
        Some(&other),
        Some(json!({"expected_revision":1})),
        StatusCode::NOT_FOUND,
    )
    .await;
    let revoked = request(
        &router,
        Method::POST,
        &format!("{path}/revoke"),
        Some(&owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(revoked["revision"], 2);
    assert!(revoked["revoked_at"].is_number());
}
