use super::*;

#[tokio::test]
async fn app_card_capabilities_are_team_scoped_redacted_and_reflect_current_binding() {
    let state = build_test_state().await;
    let (_, root) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let (owner_id, owner) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let (_, outsider) = create_auth_token_with_role_and_user_id(&state, UserRole::Operator).await;
    let (member_id, member) =
        create_auth_token_with_role_and_user_id(&state, UserRole::Viewer).await;
    let team_id = team(&state, Some(&owner_id), "planner").await;
    sqlx::query("INSERT INTO team_members(team_id, user_id, role, created_by_user_id, created_at, updated_at) VALUES (?, ?, 'member', ?, 0, 0)")
        .bind(&team_id).bind(&member_id).bind(&owner_id).execute(&state.db).await.unwrap();
    let router = crate::api::router(state.clone());
    let mut input = registration(Some(&owner_id), "card");
    input["manifest"]["tools"].as_array_mut().unwrap().push(json!({"name":"write","input_schema":{"type":"object"},"required_scopes":["write"],"replay":{"kind":"non_idempotent"}}));
    let app = request(
        &router,
        Method::POST,
        "/apps",
        Some(&root),
        Some(input.clone()),
        StatusCode::CREATED,
    )
    .await;
    let id = app["id"].as_str().unwrap();
    let grant_path = format!("/teams/{team_id}/apps/{id}");
    let binding_path = format!("/teams/{team_id}/members/planner/apps/{id}");
    request(
        &router,
        Method::PUT,
        &grant_path,
        Some(&owner),
        Some(json!({"expected_revision":0,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&owner),
        Some(json!({"expected_revision":0,"version":1,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    let card_path = "/agents/planner/.well-known/agent-card";
    let expected = json!([{"app_id":id,"name":"Example tools","version":1,"tools":["lookup"]}]);
    for token in [&owner, &member] {
        let card = request(
            &router,
            Method::GET,
            card_path,
            Some(token),
            None,
            StatusCode::OK,
        )
        .await;
        assert_eq!(card["bound_apps"], expected);
    }
    let card = request(
        &router,
        Method::GET,
        card_path,
        Some(&outsider),
        None,
        StatusCode::OK,
    )
    .await;
    assert!(card.get("bound_apps").is_none());
    input["manifest"]["tools"][0]["name"] = json!("lookup_v2");
    request(
        &router,
        Method::POST,
        &format!("/apps/{id}/versions"),
        Some(&owner),
        Some(json!({"expected_revision":1,"manifest":input["manifest"]})),
        StatusCode::CREATED,
    )
    .await;
    let card = request(
        &router,
        Method::GET,
        card_path,
        Some(&owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        card["bound_apps"], expected,
        "publication alone must not change capabilities"
    );
    request(
        &router,
        Method::PUT,
        &binding_path,
        Some(&owner),
        Some(json!({"expected_revision":1,"version":2,"scopes":["read"]})),
        StatusCode::OK,
    )
    .await;
    let card = request(
        &router,
        Method::GET,
        card_path,
        Some(&member),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(card["bound_apps"][0]["version"], 2);
    assert_eq!(card["bound_apps"][0]["tools"], json!(["lookup_v2"]));
    request(
        &router,
        Method::POST,
        &format!("{grant_path}/revoke"),
        Some(&owner),
        Some(json!({"expected_revision":1})),
        StatusCode::OK,
    )
    .await;
    let card = request(
        &router,
        Method::GET,
        card_path,
        Some(&owner),
        None,
        StatusCode::OK,
    )
    .await;
    assert!(card.get("bound_apps").is_none());
}
