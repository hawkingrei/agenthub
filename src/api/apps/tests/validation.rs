use super::*;

#[tokio::test]
async fn app_requests_are_bounded_and_validation_errors_do_not_echo_private_fields() {
    let state = build_test_state().await;
    let (owner_id, owner) = create_auth_token_with_role_and_user_id(&state, UserRole::Root).await;
    let router = crate::api::router(state.clone());
    for change in [
        json!({"endpoint":"https://private.example.test/mcp?secret=PRIVATE_APP_TOKEN","authority":"example","namespace":"one"}),
        json!({"endpoint":2,"credential_env":"PRIVATE_APP_TOKEN","authority":"example","namespace":"one"}),
    ] {
        let mut body = registration(None, "one");
        body["connection"] = change;
        request(
            &router,
            Method::POST,
            "/apps",
            Some(&owner),
            Some(body),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    for field in ["name", "manifest", "credential_override"] {
        let mut body = registration(None, "one");
        body[field] = json!("PRIVATE_APP_TOKEN");
        if field == "name" {
            body[field] = json!("x".repeat(129));
        }
        request(
            &router,
            Method::POST,
            "/apps",
            Some(&owner),
            Some(body),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    let mut body = registration(None, "one");
    body["manifest"]["tools"][0]["input_schema"] =
        json!({"$ref":"https://private.example.test/PRIVATE_APP_TOKEN"});
    request(
        &router,
        Method::POST,
        "/apps",
        Some(&owner),
        Some(body),
        StatusCode::BAD_REQUEST,
    )
    .await;
    request(
        &router,
        Method::POST,
        "/apps",
        Some(&owner),
        Some(registration(Some("missing-owner"), "one")),
        StatusCode::BAD_REQUEST,
    )
    .await;
    let mut body = registration(None, "one");
    body["name"] = json!("x".repeat(524_289));
    request(
        &router,
        Method::POST,
        "/apps",
        Some(&owner),
        Some(body),
        StatusCode::BAD_REQUEST,
    )
    .await;
    for query in ["limit=0", "limit=101", "after="] {
        request(
            &router,
            Method::GET,
            &format!("/apps?{query}"),
            Some(&owner),
            None,
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    let team_id = team(&state, Some(&owner_id), "planner").await;
    let app = request(
        &router,
        Method::POST,
        "/apps",
        Some(&owner),
        Some(registration(None, "one")),
        StatusCode::CREATED,
    )
    .await;
    let id = app["id"].as_str().unwrap();
    let path = format!("/teams/{team_id}/apps/{id}");
    for scopes in [json!([]), json!(["invalid scope"])] {
        request(
            &router,
            Method::PUT,
            &path,
            Some(&owner),
            Some(json!({"expected_revision":0,"scopes":scopes})),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    for revision in [-1, i64::MAX] {
        request(
            &router,
            Method::PUT,
            &path,
            Some(&owner),
            Some(json!({"expected_revision":revision,"scopes":["read"]})),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    request(
        &router,
        Method::PUT,
        &format!("/teams/{team_id}/members/planner/apps/{id}"),
        Some(&owner),
        Some(json!({"expected_revision":0,"scopes":["read"],"version":0})),
        StatusCode::BAD_REQUEST,
    )
    .await;
}
