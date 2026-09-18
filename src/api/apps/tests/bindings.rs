use super::*;

#[tokio::test]
async fn explicit_teamspace_owner_can_approve_and_team_owner_can_revoke_independently() {
    let state = build_test_state().await;
    let (_, root) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let (app_owner_id, app_owner) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let (team_owner_id, team_owner) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let team_id = team(&state, Some(&team_owner_id), "planner").await;
    let router = crate::api::router(state.clone());
    let app = request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(registration(Some(&app_owner_id), "shared")),
        StatusCode::CREATED,
    )
    .await;
    let id = app["id"].as_str().unwrap();
    let grant_path = format!("/teams/{team_id}/apps/{id}");
    let binding_path = format!("/teams/{team_id}/members/planner/apps/{id}");
    sqlx::query("INSERT INTO team_members(team_id, user_id, role, created_by_user_id, created_at, updated_at) VALUES (?, ?, 'owner', ?, 0, 0)")
        .bind(&team_id).bind(&app_owner_id).bind(&team_owner_id).execute(&state.db).await.unwrap();
    request(
        &router,
        Method::PUT,
        &grant_path,
        Some(&app_owner),
        Some(json!({"expected_revision":0,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    sqlx::query("UPDATE team_members SET revoked_at = 1 WHERE team_id = ? AND user_id = ?")
        .bind(&team_id)
        .bind(&app_owner_id)
        .execute(&state.db)
        .await
        .unwrap();
    request(
        &router,
        Method::PUT,
        &grant_path,
        Some(&app_owner),
        Some(json!({"expected_revision":1,"scopes":["read"]})),
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&team_owner),
        Some(json!({"expected_revision":0,"version":1,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("/apps/{id}/revoke"),
        Some(&team_owner),
        Some(json!({"expected_revision":1})),
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("/apps/{id}/revoke"),
        Some(&app_owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("{binding_path}/revoke"),
        Some(&team_owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("{grant_path}/revoke"),
        Some(&team_owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
}

#[tokio::test]
async fn team_grants_intersect_ownership_and_bind_only_explicit_approved_members() {
    let state = build_test_state().await;
    let (_, root) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let (owner_id, owner) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let (other_id, other) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let router = crate::api::router(state.clone());
    let app = request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(registration(Some(&owner_id), "bound")),
        StatusCode::CREATED,
    )
    .await;
    let id = app["id"].as_str().unwrap();
    let team_id = team(&state, Some(&owner_id), "planner").await;
    let other_team = team(&state, Some(&other_id), "reviewer").await;
    let legacy_team = team(&state, None, "worker-1").await;
    let grant_path = format!("/teams/{team_id}/apps/{id}");
    let list_path = format!("/teams/{team_id}/members/planner/apps");
    let binding_path = format!("{list_path}/{id}");
    let approval = json!({"expected_revision":0,"scopes":["read"]});
    let binding = json!({"expected_revision":0,"version":1,"scopes":["read"]});
    request(
        &router,
        Method::PUT,
        &grant_path,
        Some(&other),
        Some(approval.clone()),
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &format!("/teams/{other_team}/apps/{id}"),
        Some(&other),
        Some(approval.clone()),
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &format!("/teams/{legacy_team}/apps/{id}"),
        Some(&owner),
        Some(approval.clone()),
        StatusCode::CONFLICT,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&owner),
        Some(binding.clone()),
        StatusCode::CONFLICT,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &grant_path,
        Some(&owner),
        Some(json!({"expected_revision":0,"scopes":["admin"]})),
        StatusCode::FORBIDDEN,
    )
    .await;
    let grant = request(
        &router,
        Method::PUT,
        &grant_path,
        Some(&owner),
        Some(approval.clone()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(grant["revision"], 1);
    request(
        &router,
        Method::PUT,
        &grant_path,
        Some(&owner),
        Some(approval),
        StatusCode::CONFLICT,
    )
    .await;
    assert_eq!(
        request(
            &router,
            Method::GET,
            &list_path,
            Some(&owner),
            None,
            StatusCode::OK
        )
        .await,
        json!([])
    );
    request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&owner),
        Some(json!({"expected_revision":0,"version":1,"scopes":["write"]})),
        StatusCode::FORBIDDEN,
    )
    .await;
    let bound = request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&owner),
        Some(binding.clone()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(bound["version"], 1);
    request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&owner),
        Some(binding),
        StatusCode::CONFLICT,
    )
    .await;
    assert_eq!(
        request(
            &router,
            Method::GET,
            &list_path,
            Some(&owner),
            None,
            StatusCode::OK
        )
        .await,
        json!([bound])
    );
    request(
        &router,
        Method::GET,
        &grant_path,
        Some(&other),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::GET,
        &list_path,
        Some(&other),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    request(
        &router,
        Method::GET,
        &format!("/teams/{team_id}/members/missing/apps"),
        Some(&owner),
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    // Team membership permits inspection but does not confer ownership or App approval authority.
    sqlx::query("INSERT INTO team_members(team_id, user_id, role, created_by_user_id, created_at, updated_at) VALUES (?, ?, 'member', ?, 0, 0)")
        .bind(&team_id).bind(&other_id).bind(&owner_id).execute(&state.db).await.unwrap();
    request(
        &router,
        Method::GET,
        &grant_path,
        Some(&other),
        None,
        StatusCode::OK,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&other),
        Some(json!({"expected_revision":1,"version":1,"scopes":["read"]})),
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("{binding_path}/revoke"),
        Some(&other),
        Some(json!({"expected_revision":1})),
        StatusCode::FORBIDDEN,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("{grant_path}/revoke"),
        Some(&other),
        Some(json!({"expected_revision":1})),
        StatusCode::FORBIDDEN,
    )
    .await;
    let revoked = request(
        &router,
        Method::POST,
        &format!("{binding_path}/revoke"),
        Some(&owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
    assert!(revoked["revoked_at"].is_number());
    request(
        &router,
        Method::POST,
        &format!("{binding_path}/revoke"),
        Some(&owner),
        Some(json!({"expected_revision":1})),
        StatusCode::CONFLICT,
    )
    .await;
    request(
        &router,
        Method::POST,
        &format!("/apps/{id}/revoke"),
        Some(&owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
    let revoked = request(
        &router,
        Method::POST,
        &format!("{grant_path}/revoke"),
        Some(&owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
    assert!(revoked["revoked_at"].is_number());
    assert_eq!(
        request(
            &router,
            Method::GET,
            &grant_path,
            Some(&owner),
            None,
            StatusCode::OK
        )
        .await,
        revoked
    );
}
