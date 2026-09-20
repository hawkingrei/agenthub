use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionKind, RequestPermissionOutcome, RequestPermissionRequest,
    ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
};
use agenthub_rara::{PendingInputKind, PlanDecision, ShellDecision};
use chrono::Utc;
use serde_json::json;

use super::*;

pub(super) struct LivePermission {
    id: String,
    pending: PendingInput,
    cancellation: CancellationToken,
}

impl LivePermission {
    pub(super) fn turn_id(&self) -> &str {
        &self.pending.turn_id
    }

    pub(super) async fn cancel(
        self,
        permissions: &agenthub_acp::AcpPermissionService,
    ) -> anyhow::Result<()> {
        self.cancellation.cancel();
        permissions.mark_timeout(&self.id, None).await
    }
}

impl RaraHandle {
    pub(super) async fn expire_obsolete_permission(
        &self,
        pending: Option<&PendingInput>,
    ) -> anyhow::Result<()> {
        let mut permission = self.permission.lock().await;
        if permission
            .as_ref()
            .is_some_and(|active| pending != Some(&active.pending))
        {
            let active = permission.take().expect("active permission");
            active.cancel(&self.permissions).await?;
        }
        Ok(())
    }

    pub(crate) async fn expire_permissions(&self) -> anyhow::Result<()> {
        self.expire_obsolete_permission(None).await
    }

    pub(super) async fn request_permission(
        &self,
        pending: PendingInput,
        tool_call_id: String,
    ) -> anyhow::Result<()> {
        let (title, raw_input, options) = match &pending.kind {
            PendingInputKind::User { .. } => return Ok(()),
            PendingInputKind::Plan { plan, .. } => (
                "Plan approval",
                json!({"plan":plan}),
                vec![
                    option("approve", "Approve plan", PermissionOptionKind::AllowOnce),
                    option(
                        "continue_planning",
                        "Continue planning",
                        PermissionOptionKind::RejectOnce,
                    ),
                    option("reject", "Reject plan", PermissionOptionKind::RejectOnce),
                ],
            ),
            PendingInputKind::Shell { request, .. } => (
                "Shell approval",
                request.clone(),
                vec![
                    option("once", "Allow once", PermissionOptionKind::AllowOnce),
                    option(
                        "prefix",
                        "Allow command prefix",
                        PermissionOptionKind::AllowAlways,
                    ),
                    option("always", "Always allow", PermissionOptionKind::AllowAlways),
                    option("deny", "Deny command", PermissionOptionKind::RejectOnce),
                ],
            ),
        };
        let mut active = self.permission.lock().await;
        if active
            .as_ref()
            .is_some_and(|active| active.pending == pending)
        {
            return Ok(());
        }
        anyhow::ensure!(
            active.is_none(),
            "direct permission ownership was not retired"
        );
        let request = RequestPermissionRequest::new(
            self.stream.native_session_id().to_owned(),
            ToolCallUpdate::new(
                tool_call_id.clone(),
                ToolCallUpdateFields::new()
                    .title(title.to_owned())
                    .status(ToolCallStatus::Pending)
                    .raw_input(raw_input),
            ),
            options,
        );
        let (id, response) = self
            .permissions
            .create_request(
                &self.agent_id,
                self.store.local_session_id(),
                &request,
                None,
            )
            .await?;
        let cancellation = CancellationToken::new();
        *active = Some(LivePermission {
            id: id.clone(),
            pending: pending.clone(),
            cancellation: cancellation.clone(),
        });
        drop(active);
        let options: Vec<_> = request
            .options
            .iter()
            .map(agenthub_acp::AcpPermissionOption::from)
            .collect();
        self.emit_history(json!({"type":"permission_request", "permission_id":id,
            "session_id":self.stream.native_session_id(), "tool_call_id":tool_call_id,
            "options":options, "created_at":Utc::now().timestamp()}))
            .await?;
        let runtime = self.clone();
        self.tasks
            .spawn_runtime_task(format!("direct-permission:{id}"), async move {
                let result = runtime
                    .wait_permission(id, pending, tool_call_id, response, cancellation)
                    .await;
                if result.is_err() {
                    tracing::warn!("direct permission answer failed; retiring the owned runtime");
                    runtime.client.abort();
                }
                Ok(())
            })?;
        Ok(())
    }

    async fn wait_permission(
        &self,
        id: String,
        pending: PendingInput,
        tool_call_id: String,
        response: tokio::sync::oneshot::Receiver<RequestPermissionOutcome>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<()> {
        let timeout = agenthub_acp::acp_permission_review_timeout();
        let outcome = tokio::select! {
            biased;
            _ = cancellation.cancelled() => None,
            _ = self.client.closed() => None,
            response = tokio::time::timeout(timeout, response) => Some(response.ok().and_then(Result::ok)),
        };
        let Some(outcome) = outcome else {
            self.permissions.mark_timeout(&id, None).await?;
            self.emit_history(json!({"type":"permission_timeout", "permission_id":id,
                "tool_call_id":tool_call_id,"responded_at":Utc::now().timestamp()}))
                .await?;
            return Ok(());
        };
        let timed_out = outcome.is_none();
        let outcome = outcome.unwrap_or(RequestPermissionOutcome::Cancelled);
        if timed_out {
            self.permissions.mark_timeout(&id, Some(&outcome)).await?;
        }
        self.emit_history(
            json!({"type":if timed_out {"permission_timeout"} else {"permission_response"},
            "permission_id":id, "tool_call_id":tool_call_id,"outcome":outcome,
            "responded_at":Utc::now().timestamp()}),
        )
        .await?;
        let _gate = self.input_gate.lock().await;
        self.await_admitted_events().await?;
        if cancellation.is_cancelled() || self.state.read().await.pending.as_ref() != Some(&pending)
        {
            self.emit_history(json!({"type":"permission_control_expired","permission_id":id}))
                .await?;
            return Ok(());
        }
        let request = decision(&pending, &outcome)?;
        let request_id = Uuid::now_v7().to_string();
        let frame = request.frame(
            self.store.runtime_id(),
            &request_id,
            Some(self.stream.native_session_id()),
        )?;
        let result = receipts::submit(
            &self.client,
            &self.store,
            frame,
            receipts::kind(request.kind()),
            Some(self.stream.native_session_id()),
            request.expected_turn_id(),
        )
        .await;
        if let Ok(ack) = &result {
            self.record_ack_cursor(ack);
        }
        if let Some(receipt) = self.store.request_receipt(&request_id).await? {
            self.emit_history(json!({"type":"permission_control_receipt", "permission_id":id,
                "receipt":receipt, "meta":{"provider_runtime":{"provider":"rara",
                    "runtime_id":self.store.runtime_id(),"native_session_id":self.stream.native_session_id(),
                    "request_id":request_id}}})).await?;
        }
        // A rejected or uncertain answer is not approval. Never replace it with a new send.
        anyhow::ensure!(
            matches!(result?, RuntimeRequestAck::Accepted { .. }),
            "direct runtime did not accept the permission answer"
        );
        Ok(())
    }
}

fn option(id: &'static str, name: &str, kind: PermissionOptionKind) -> PermissionOption {
    PermissionOption::new(id, name, kind)
}

fn decision(
    pending: &PendingInput,
    outcome: &RequestPermissionOutcome,
) -> anyhow::Result<ControlRequest> {
    let selected = match outcome {
        RequestPermissionOutcome::Selected(selected) => Some(selected.option_id.0.as_ref()),
        _ => None,
    };
    // The shared response endpoint accepts arbitrary option IDs. Only these explicit
    // choices can authorize execution; cancellation and unknown IDs deny it.
    Ok(match pending.kind {
        PendingInputKind::Plan { .. } => ControlRequest::PlanAnswer {
            turn_id: pending.turn_id.clone(),
            decision: match selected {
                Some("approve") => PlanDecision::Approve,
                Some("continue_planning") => PlanDecision::ContinuePlanning,
                _ => PlanDecision::Reject,
            },
            feedback: None,
        },
        PendingInputKind::Shell { .. } => ControlRequest::ShellAnswer {
            turn_id: pending.turn_id.clone(),
            decision: match selected {
                Some("once") => ShellDecision::Once,
                Some("prefix") => ShellDecision::Prefix,
                Some("always") => ShellDecision::Always,
                _ => ShellDecision::Deny,
            },
        },
        PendingInputKind::User { .. } => anyhow::bail!("a user question is not a permission"),
    })
}
