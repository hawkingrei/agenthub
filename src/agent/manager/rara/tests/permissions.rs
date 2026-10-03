use agent_client_protocol::schema::v1::{RequestPermissionOutcome, SelectedPermissionOutcome};

use super::*;

async fn start_permission(
    fixture: &Fixture,
) -> (
    String,
    tokio::sync::broadcast::Receiver<crate::agent::AgentOutput>,
) {
    let session = fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let mut receiver = fixture
        .manager
        .subscribe_output(&fixture.agent_id)
        .await
        .unwrap();
    fixture
        .manager
        .send_input(
            &fixture.agent_id,
            "request approval",
            Some("prompt"),
            Some(&session),
        )
        .await
        .unwrap();
    let request = input::output(&mut receiver, "permission_request").await;
    let id = request["permission_id"].as_str().unwrap().to_owned();
    let call_id: String =
        sqlx::query_scalar("SELECT tool_call_id FROM acp_permission_requests WHERE id = ?")
            .bind(&id)
            .fetch_one(&fixture.manager.db)
            .await
            .unwrap();
    assert_eq!(call_id, "direct:native-session:tool:event-3");
    (id, receiver)
}

async fn choose(fixture: &Fixture, id: &str, option: &str) {
    fixture
        .manager
        .permissions
        .respond(
            id,
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option.to_owned())),
            Some(option.to_owned()),
            Some("operator".into()),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn native_permissions_preserve_operator_choice_and_fenced_ack_separately() {
    for (scenario, option, decision, method) in [
        ("permission_shell", "once", "once", "answer_shell_approval"),
        (
            "permission_shell",
            "prefix",
            "prefix",
            "answer_shell_approval",
        ),
        (
            "permission_shell",
            "always",
            "always",
            "answer_shell_approval",
        ),
        (
            "permission_shell",
            "deny",
            "suggestion",
            "answer_shell_approval",
        ),
        (
            "permission_shell",
            "unexpected",
            "suggestion",
            "answer_shell_approval",
        ),
        (
            "permission_plan",
            "approve",
            "approve",
            "answer_plan_approval",
        ),
        (
            "permission_plan",
            "continue_planning",
            "continue_planning",
            "answer_plan_approval",
        ),
        (
            "permission_plan",
            "reject",
            "reject",
            "answer_plan_approval",
        ),
        (
            "permission_plan",
            "unexpected",
            "reject",
            "answer_plan_approval",
        ),
    ] {
        let fixture = Fixture::new(scenario).await;
        let (id, mut receiver) = start_permission(&fixture).await;
        choose(&fixture, &id, option).await;
        let receipt = input::output(&mut receiver, "permission_control_receipt").await;
        assert_eq!(receipt["receipt"]["status"], "accepted");
        assert_eq!(receipt["receipt"]["expected_turn_id"], "turn-1");
        assert_eq!(receipt["receipt"]["ack"]["turn_id"], "turn-2");
        let selected: String = sqlx::query_scalar(
            "SELECT selected_option_id FROM acp_permission_requests WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&fixture.manager.db)
        .await
        .unwrap();
        assert_eq!(selected, option);
        let requests = fixture.input_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1]["payload"]["expected_turn_id"], "turn-1");
        let operation = &requests[1]["payload"]["envelope"]["request"]["payload"];
        assert_eq!(operation["type"], method);
        assert_eq!(operation["payload"]["decision"], decision);
        fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
        fixture.finish().await;
    }
}

#[tokio::test]
async fn transport_loss_expires_native_permission_callbacks() {
    let fixture = Fixture::new("permission_drop").await;
    let (id, _) = start_permission(&fixture).await;
    std::fs::write(fixture.directory.join("drop-permission"), "drop").unwrap();
    fixture.assert_clean().await;
    choose(&fixture, &id, "always").await;
    let status: String =
        sqlx::query_scalar("SELECT status FROM acp_permission_requests WHERE id = ?")
            .bind(id)
            .fetch_one(&fixture.manager.db)
            .await
            .unwrap();
    assert_eq!(status, "timeout");
    assert_eq!(fixture.input_requests().len(), 1);
    fixture.finish().await;
}

#[tokio::test]
async fn superseded_permission_cannot_approve_the_replacement_turn() {
    let fixture = Fixture::new("permission_stale").await;
    let (old_id, mut receiver) = start_permission(&fixture).await;
    std::fs::write(fixture.directory.join("replace-permission"), "replace").unwrap();
    let next = input::output(&mut receiver, "permission_request").await;
    assert_ne!(next["permission_id"], old_id);
    choose(&fixture, &old_id, "always").await;
    let status: String =
        sqlx::query_scalar("SELECT status FROM acp_permission_requests WHERE id = ?")
            .bind(old_id)
            .fetch_one(&fixture.manager.db)
            .await
            .unwrap();
    assert_eq!(status, "timeout");
    assert_eq!(fixture.input_requests().len(), 1);
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
async fn rejected_permission_answer_is_not_treated_as_execution_approval() {
    let fixture = Fixture::new("permission_reject").await;
    let (id, mut receiver) = start_permission(&fixture).await;
    choose(&fixture, &id, "once").await;
    let receipt = input::output(&mut receiver, "permission_control_receipt").await;
    assert_eq!(receipt["receipt"]["status"], "rejected");
    fixture.assert_clean().await;
    assert_eq!(fixture.input_requests().len(), 2);
    fixture.finish().await;
}

#[tokio::test]
async fn permission_service_timeout_sends_one_explicit_native_denial() {
    let fixture = Fixture::new("permission_shell").await;
    let (id, mut receiver) = start_permission(&fixture).await;
    fixture
        .manager
        .permissions
        .mark_timeout(&id, None)
        .await
        .unwrap();
    let receipt = input::output(&mut receiver, "permission_control_receipt").await;
    assert_eq!(receipt["receipt"]["status"], "accepted");
    let requests = fixture.input_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1]["payload"]["envelope"]["request"]["payload"]["payload"]["decision"],
        "suggestion"
    );
    let status: String =
        sqlx::query_scalar("SELECT status FROM acp_permission_requests WHERE id = ?")
            .bind(id)
            .fetch_one(&fixture.manager.db)
            .await
            .unwrap();
    assert_eq!(status, "timeout");
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
async fn cancel_and_interrupt_expire_the_owned_waiting_permission() {
    for interrupt in [false, true] {
        let fixture = Fixture::new("permission_shell").await;
        let (id, _) = start_permission(&fixture).await;
        if interrupt {
            let input = fixture.manager.inner.read().await[&fixture.agent_id]
                .input
                .clone();
            let AgentInput::Rara(runtime) = input else {
                panic!("native runtime");
            };
            runtime.stop_turn(true).await.unwrap();
        } else {
            fixture.manager.cancel_acp(&fixture.agent_id).await.unwrap();
        }
        choose(&fixture, &id, "always").await;
        let status: String =
            sqlx::query_scalar("SELECT status FROM acp_permission_requests WHERE id = ?")
                .bind(&id)
                .fetch_one(&fixture.manager.db)
                .await
                .unwrap();
        assert_eq!(status, "timeout");
        let requests = fixture.input_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1]["payload"]["expected_turn_id"], "turn-1");
        assert_eq!(
            requests[1]["payload"]["envelope"]["request"]["payload"]["type"],
            if interrupt {
                "interrupt_current_turn"
            } else {
                "cancel_current_turn"
            }
        );
        let request_id = requests[1]["payload"]["envelope"]["request_id"]
            .as_str()
            .unwrap();
        assert_eq!(fixture.input_receipt(request_id).await.0, "accepted");
        fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
        fixture.finish().await;
    }
}
