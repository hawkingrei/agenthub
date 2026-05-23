use super::*;

#[tokio::test]
async fn update_team_spec_if_unchanged_detects_conflict_and_updates_on_match() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());

    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "update-team-spec".to_string(),
            description: Some("optimistic lock".to_string()),
            spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        })
        .await
        .expect("create team");

    let conflicted = manager
        .update_team_spec_if_unchanged(
            &team.id,
            team.updated_at - 1,
            json!({"entrypoint":"stale","members":[{"member_id":"planner"}]}),
        )
        .await
        .expect("stale update should not fail");
    assert!(conflicted.is_none());

    let updated = manager
        .update_team_spec_if_unchanged(
            &team.id,
            team.updated_at,
            json!({"entrypoint":"updated","members":[{"member_id":"planner"}]}),
        )
        .await
        .expect("matching update")
        .expect("team should be updated");
    assert_eq!(updated.spec["entrypoint"], json!("updated"));
}

#[tokio::test]
async fn update_team_spec_if_unchanged_returns_not_found_for_missing_team() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);

    let err = manager
        .update_team_spec_if_unchanged(
            "missing-team",
            0,
            json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        )
        .await
        .expect_err("missing team should fail");

    assert!(matches!(
        err.downcast_ref::<sqlx::Error>(),
        Some(sqlx::Error::RowNotFound)
    ));
}
