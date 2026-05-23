use super::*;

#[tokio::test]
async fn create_team_channel_creates_bootstrap_conversation_and_hides_it_from_task_list() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-create".to_string(),
            description: Some("verify channel bootstrap records".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let channel = manager
        .create_channel(&team.id, "review", Some("Review queue"), "coordinator")
        .await
        .expect("create review channel");
    assert_eq!(channel.team_id, team.id);
    assert_eq!(channel.channel_id, "review");
    assert_ne!(channel.conversation_id, "review");
    assert_eq!(channel.description.as_deref(), Some("Review queue"));
    assert_eq!(channel.created_by_actor_id, "coordinator");

    let conversation = sqlx::query(
        r#"
        SELECT c.id, c.task_id, t.context_json
        FROM team_conversations c
        INNER JOIN team_tasks t ON t.id = c.task_id
        WHERE c.team_id = ?1 AND c.id = ?2
        LIMIT 1
        "#,
    )
    .bind(&team.id)
    .bind(&channel.conversation_id)
    .fetch_one(&db)
    .await
    .expect("fetch review conversation");
    assert_eq!(conversation.get::<String, _>("id"), channel.conversation_id);
    assert_eq!(conversation.get::<String, _>("task_id"), channel.task_id);
    let context_json: Value = serde_json::from_str(&conversation.get::<String, _>("context_json"))
        .expect("parse context");
    assert_eq!(context_json["bootstrap_kind"], "team_channel");
    assert_eq!(context_json["bootstrap_source"], "coordinator_created");
    assert_eq!(context_json["channel_id"], "review");
    assert_eq!(context_json["description"], "Review queue");

    let listed = manager
        .list_tasks_with_query(TeamTaskListQuery {
            team_id: Some(team.id.clone()),
            limit: 20,
            include_shared_thread: false,
            ..TeamTaskListQuery::default()
        })
        .await
        .expect("list visible tasks");
    assert!(
        listed.is_empty(),
        "channel bootstrap tasks should stay hidden"
    );
}

#[tokio::test]
async fn list_team_channels_returns_non_default_channels_in_creation_order() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-list".to_string(),
            description: Some("verify channel listing".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team");

    manager
        .create_channel(&team.id, "review", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel");
    manager
        .create_channel(&team.id, "research", Some("Research lane"), "coordinator")
        .await
        .expect("create research channel");

    let listed = manager
        .list_channels(&team.id)
        .await
        .expect("list team channels");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].channel_id, "review");
    assert_eq!(listed[0].description.as_deref(), Some("Review lane"));
    assert_eq!(listed[1].channel_id, "research");
    assert_eq!(listed[1].description.as_deref(), Some("Research lane"));
}

#[tokio::test]
async fn list_team_channels_ignores_bootstrap_rows_with_blank_channel_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-invalid".to_string(),
            description: Some("ignore invalid bootstrap rows".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team");

    manager
        .create_channel(&team.id, "review", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel");

    let task_id = Uuid::new_v4().to_string();
    let conversation_id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    sqlx::query(
        r#"
        INSERT INTO team_tasks (
            id,
            team_id,
            title,
            status,
            priority,
            created_by_actor_id,
            assigned_member_id,
            context_json,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, 'open', 'medium', ?4, NULL, ?5, ?6, ?6)
        "#,
    )
    .bind(&task_id)
    .bind(&team.id)
    .bind("invalid-bootstrap")
    .bind("coordinator")
    .bind(
        json!({
            "bootstrap_kind": "team_channel",
            "channel_id": "   ",
            "description": "broken bootstrap row"
        })
        .to_string(),
    )
    .bind(now)
    .execute(&db)
    .await
    .expect("insert invalid bootstrap task");
    sqlx::query(
        r#"
        INSERT INTO team_conversations (
            id,
            team_id,
            task_id,
            mode,
            topic,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, 'group_chat', ?4, ?5, ?5)
        "#,
    )
    .bind(&conversation_id)
    .bind(&team.id)
    .bind(&task_id)
    .bind("invalid-bootstrap")
    .bind(now)
    .execute(&db)
    .await
    .expect("insert invalid bootstrap conversation");

    let listed = manager
        .list_channels(&team.id)
        .await
        .expect("list channels");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].channel_id, "review");
}

#[tokio::test]
async fn create_team_channel_allows_same_channel_id_in_different_teams() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team_a = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-a".to_string(),
            description: Some("team a".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team a");
    let team_b = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-b".to_string(),
            description: Some("team b".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team b");

    let review_a = manager
        .create_channel(&team_a.id, "review", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel for team a");
    let review_b = manager
        .create_channel(&team_b.id, "review", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel for team b");

    assert_ne!(review_a.conversation_id, review_b.conversation_id);

    let conversation_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM team_tasks t
        INNER JOIN team_conversations c ON c.task_id = t.id
        WHERE lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) = 'team_channel'
          AND lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) = 'review'
        "#,
    )
    .fetch_one(&db)
    .await
    .expect("count review channel bootstraps");
    assert_eq!(conversation_count, 2);
}

#[tokio::test]
async fn create_team_channel_canonicalizes_case_and_rejects_same_team_duplicates() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-case".to_string(),
            description: Some("verify channel canonicalization".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team");

    let channel = manager
        .create_channel(&team.id, " Review ", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel");
    assert_eq!(channel.channel_id, "review");

    let duplicate = manager
        .create_channel(
            &team.id,
            "REVIEW",
            Some("Duplicate review lane"),
            "coordinator",
        )
        .await
        .expect_err("duplicate review channel should fail");
    assert!(
        duplicate
            .to_string()
            .contains("channel 'review' already exists")
    );
}

#[tokio::test]
async fn create_team_channel_rejects_empty_creator_actor_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db);
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-empty-creator".to_string(),
            description: Some("verify creator validation".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team");

    let err = manager
        .create_channel(&team.id, "review", Some("Review lane"), "   ")
        .await
        .expect_err("empty creator should fail");
    assert!(err.to_string().contains("created_by_actor_id is required"));
}

#[tokio::test]
async fn delete_team_channel_cleans_bootstrap_rows_and_rejects_all() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-channel-delete".to_string(),
            description: Some("verify channel deletion cleanup".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let cannot_delete_all = manager
        .delete_channel(&team.id, "all")
        .await
        .expect_err("shared channel should be reserved");
    assert!(
        cannot_delete_all
            .to_string()
            .contains("channel_id 'all' cannot be deleted")
    );

    let channel = manager
        .create_channel(&team.id, "research", Some("Research lane"), "coordinator")
        .await
        .expect("create research channel");

    let root_message_id = insert_team_conversation_message(
        &db,
        &channel.conversation_id,
        &channel.task_id,
        "coordinator",
        json!({"text":"Investigate issue"}),
    )
    .await;
    sqlx::query(
        r#"
        INSERT INTO team_channel_message_replicas (
            authority_message_id, correlation_id, run_id, team_id, conversation_id, task_id, channel_id, from_actor_id, source_node_id, payload_json, stored_at
        )
        VALUES (?1, 'corr-delete-replica', 'run-1', ?2, ?3, ?4, ?5, 'coordinator', 'main', '{"text":"Investigate issue","correlation_id":"corr-delete-replica"}', ?6)
        "#,
    )
    .bind(root_message_id)
    .bind(&team.id)
    .bind(&channel.conversation_id)
    .bind(&channel.task_id)
    .bind(&channel.channel_id)
    .bind(Utc::now().timestamp())
    .execute(&db)
    .await
    .expect("insert channel replica");

    let deleted = manager
        .delete_channel(&team.id, "research")
        .await
        .expect("delete research channel");
    assert_eq!(deleted.channel_id, "research");
    assert_eq!(deleted.task_id, channel.task_id);

    let remaining_conversations =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_conversations WHERE id = ?1")
            .bind(&channel.conversation_id)
            .fetch_one(&db)
            .await
            .expect("count conversations");
    let remaining_tasks =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_tasks WHERE id = ?1")
            .bind(&channel.task_id)
            .fetch_one(&db)
            .await
            .expect("count tasks");
    let remaining_messages = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_conversation_messages WHERE conversation_id = ?1",
    )
    .bind(&channel.conversation_id)
    .fetch_one(&db)
    .await
    .expect("count messages");
    let remaining_replicas = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_channel_message_replicas WHERE conversation_id = ?1",
    )
    .bind(&channel.conversation_id)
    .fetch_one(&db)
    .await
    .expect("count replicas");

    assert_eq!(remaining_conversations, 0);
    assert_eq!(remaining_tasks, 0);
    assert_eq!(remaining_messages, 0);
    assert_eq!(remaining_replicas, 0);
}

#[tokio::test]
async fn delete_team_channel_returns_canonical_channel_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-delete-case".to_string(),
            description: Some("verify delete canonicalization".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team");

    manager
        .create_channel(&team.id, "Review", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel");
    let deleted = manager
        .delete_channel(&team.id, " REVIEW ")
        .await
        .expect("delete review channel");

    assert_eq!(deleted.channel_id, "review");
}

#[tokio::test]
async fn delete_team_channel_does_not_touch_other_team_same_channel_id() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team_a = manager
        .create_team(TeamDefinitionConfig {
            name: "team-delete-a".to_string(),
            description: Some("team a".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team a");
    let team_b = manager
        .create_team(TeamDefinitionConfig {
            name: "team-delete-b".to_string(),
            description: Some("team b".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[{"member_id":"coordinator","role":"coordinator"}]
            }),
        })
        .await
        .expect("create team b");

    let review_a = manager
        .create_channel(&team_a.id, "review", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel for team a");
    let review_b = manager
        .create_channel(&team_b.id, "review", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel for team b");

    manager
        .delete_channel(&team_a.id, "review")
        .await
        .expect("delete review channel for team a");

    let surviving_conversation =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_conversations WHERE id = ?1")
            .bind(&review_b.conversation_id)
            .fetch_one(&db)
            .await
            .expect("count surviving conversation");
    let surviving_task =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_tasks WHERE id = ?1")
            .bind(&review_b.task_id)
            .fetch_one(&db)
            .await
            .expect("count surviving task");
    let deleted_conversation =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_conversations WHERE id = ?1")
            .bind(&review_a.conversation_id)
            .fetch_one(&db)
            .await
            .expect("count deleted conversation");

    assert_eq!(surviving_conversation, 1);
    assert_eq!(surviving_task, 1);
    assert_eq!(deleted_conversation, 0);
}

#[tokio::test]
async fn open_team_thread_supports_shared_and_custom_channels() {
    let db = setup_test_db().await;
    let manager = TeamManager::new(db.clone());
    let team = manager
        .create_team(TeamDefinitionConfig {
            name: "team-open-thread".to_string(),
            description: Some("verify open thread routes".to_string()),
            spec: json!({
                "entrypoint":"coordinator",
                "members":[
                    {"member_id":"coordinator","role":"coordinator"},
                    {"member_id":"worker","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create team");

    let (shared_task, shared_conversation) = manager
        .create_task(
            &team.id,
            "all",
            "coordinator",
            json!({"bootstrap_kind":"shared_thread"}),
            "group_chat",
            Some("all"),
        )
        .await
        .expect("create shared thread target");
    let shared_conversation_id = shared_conversation.id;
    let shared_task_id = shared_task.id;
    let shared_root_message_id = insert_team_conversation_message(
        &db,
        &shared_conversation_id,
        &shared_task_id,
        "coordinator",
        json!({"text":"Shared update"}),
    )
    .await;

    let shared_thread = manager
        .open_thread(&team.id, "all", shared_root_message_id)
        .await
        .expect("open shared thread");
    assert_eq!(shared_thread.channel_id, "all");
    assert_eq!(shared_thread.conversation_id, shared_conversation_id);
    assert_eq!(shared_thread.task_id, shared_task_id);
    assert_eq!(shared_thread.root_message_id, shared_root_message_id);
    assert_eq!(shared_thread.thread_id, shared_root_message_id.to_string());

    let channel = manager
        .create_channel(&team.id, "review", Some("Review lane"), "coordinator")
        .await
        .expect("create review channel");
    let review_root_message_id = insert_team_conversation_message(
        &db,
        &channel.conversation_id,
        &channel.task_id,
        "coordinator",
        json!({"text":"Please review"}),
    )
    .await;

    let review_thread = manager
        .open_thread(&team.id, "ReViEw", review_root_message_id)
        .await
        .expect("open review thread");
    assert_eq!(review_thread.channel_id, "review");
    assert_eq!(review_thread.conversation_id, channel.conversation_id);
    assert_eq!(review_thread.task_id, channel.task_id);
    assert_eq!(review_thread.root_message_id, review_root_message_id);
    assert_eq!(review_thread.thread_id, review_root_message_id.to_string());

    let review_reply = manager
        .reply_thread(
            &team.id,
            " review ",
            review_root_message_id,
            "worker",
            "This should stay in the review thread.",
            &[],
        )
        .await
        .expect("reply to review thread");
    assert_eq!(
        review_reply.thread.thread_id,
        review_root_message_id.to_string()
    );
    assert_eq!(review_reply.message.task_id, channel.task_id);
    assert_eq!(
        review_reply.message.conversation_id,
        channel.conversation_id
    );
    assert_eq!(review_reply.message.from_actor_id, "worker");
    assert_eq!(review_reply.message.route, "team_thread_reply");
    assert_eq!(review_reply.message.payload["type"], json!("chat_message"));
    assert_eq!(
        review_reply.message.payload["thread_root_message_id"],
        json!(review_root_message_id)
    );
    assert_eq!(
        review_reply.message.payload["text"],
        json!("This should stay in the review thread.")
    );
}
