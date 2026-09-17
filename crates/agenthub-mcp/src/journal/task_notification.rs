use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{McpTaskAuthority, McpTaskReceipt},
};
use agenthub_db::mcp_operations::McpTaskNotificationPermit;

use super::*;

impl JournaledMcpClient {
    pub(crate) async fn authorize_task_notifications(
        &self,
        executor: &LoopReservation,
        authority: &McpTaskAuthority,
        receipt: &McpTaskReceipt,
    ) -> Result<McpTaskNotificationPermit, McpCallError> {
        self.journal
            .authorize_task_notifications(executor, authority, receipt, now())
            .await
            .map_err(journal_error)
    }

    pub(crate) async fn record_task_notification(
        &self,
        permit: &McpTaskNotificationPermit,
        message: &Value,
    ) -> Result<(), McpCallError> {
        let observation = crate::task::notification_observation(permit.receipt(), message)?;
        if permit.receipt().version
            == agenthub_agent_domain::mcp_operations::McpTaskVersion::July2026
            && message["params"]["status"] == "completed"
        {
            self.validate_native_result(permit.tool_name(), &message["params"]["result"])?;
        }
        let mut fact = message["params"].clone();
        // A reconnect may carry the same observation under a new subscription RPC ID.
        if let Some(meta) = fact.get_mut("_meta").and_then(Value::as_object_mut) {
            meta.remove("io.modelcontextprotocol/subscriptionId");
            if meta.is_empty() {
                fact.as_object_mut().unwrap().remove("_meta");
            }
        }
        let response_digest = digest("mcp-task-notification-v1", &fact)?;
        self.journal
            .record_task_notification(
                permit,
                &response_digest,
                observation.outcome.as_ref(),
                observation.inputs.as_deref(),
                now(),
            )
            .await
            .map_err(journal_error)
    }
}
