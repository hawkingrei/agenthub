use super::*;
use tonic::{Response, Status};

#[tokio::test]
async fn loop_control_observations_record_only_safe_status_and_keep_unknown_results() {
    let (state, service, authz, run, reservation) = fixture().await;
    let credential = token(&authz, "reviewer", &run.id, &reservation);
    let metadata = authenticated_request((), &credential).metadata().clone();
    for (name, code) in [
        ("test_denied", tonic::Code::PermissionDenied),
        ("test_timeout", tonic::Code::DeadlineExceeded),
    ] {
        let error = service
            .complete_control_request(&metadata, name, async move {
                Err::<Response<()>, _>(Status::new(code, "private-output-and-arguments"))
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), code);
    }
    let store = LoopStore::new(state.db.clone());
    let id = reservation.activation_id.as_deref().unwrap();
    let page = store
        .activation_tool_history(&run.team_id, "reviewer", id, None, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.tools.len(), 2);
    assert_eq!(page.tools[0].tool_name, "test_denied");
    assert_eq!(page.tools[0].status.as_str(), "failed");
    assert_eq!(page.tools[1].status.as_str(), "outcome_unknown");
    assert!(
        page.tools
            .iter()
            .all(|tool| tool.surface.as_str() == "control_rpc" && tool.duration_ms.is_some())
    );
    assert!(
        !serde_json::to_string(&page)
            .unwrap()
            .contains("private-output")
    );
    let mut stale = reservation.clone();
    stale.generation += 1;
    let credential = token(&authz, "reviewer", &run.id, &stale);
    let metadata = authenticated_request((), &credential).metadata().clone();
    let operation_service = service.clone();
    let operation_metadata = metadata.clone();
    assert_eq!(
        service
            .complete_control_request(&metadata, "test_stale", async move {
                operation_service
                    .authenticate_execution(&operation_metadata, false)
                    .await?;
                Ok(Response::new(()))
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::PermissionDenied
    );
    assert_eq!(
        store
            .activation_tool_history(&run.team_id, "reviewer", id, None, 100)
            .await
            .unwrap()
            .unwrap()
            .tools
            .len(),
        2
    );
}
