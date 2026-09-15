use agenthub_agent_domain::loop_scheduling::{
    LoopRegistration, LoopRegistrationDetail, LoopRegistrationPage, LoopRegistrationReceipt,
    LoopScheduleRequest,
};

use super::super::proto::agenthub::internal::v1::{
    GetLoopScheduleRequest, ListLoopSchedulesRequest, RegisterLoopScheduleRequest,
    RevokeLoopScheduleRequest,
};
use super::{
    InternalGrpcMailboxClient, map_grpc_status_anyhow, parse_json_response,
    timeout_internal_grpc_call,
};

impl InternalGrpcMailboxClient {
    pub(crate) async fn register_loop_schedule(
        &self,
        member: Option<&str>,
        intent: &LoopScheduleRequest,
    ) -> anyhow::Result<LoopRegistrationReceipt> {
        intent.validate()?;
        let response = timeout_internal_grpc_call(self.client().register_loop_schedule(
            self.control_request(RegisterLoopScheduleRequest {
                member_id: member.unwrap_or_default().into(),
                request_json: serde_json::to_string(intent)?,
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.receipt_json, "receipt_json")
    }

    pub(crate) async fn list_loop_schedules(
        &self,
        member: Option<&str>,
        after: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<LoopRegistrationPage> {
        let response = timeout_internal_grpc_call(self.client().list_loop_schedules(
            self.control_request(ListLoopSchedulesRequest {
                member_id: member.unwrap_or_default().into(),
                after_registration_id: after.unwrap_or_default().into(),
                limit,
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.page_json, "page_json")
    }

    pub(crate) async fn get_loop_schedule(
        &self,
        id: &str,
        after: Option<i64>,
        limit: u32,
    ) -> anyhow::Result<LoopRegistrationDetail> {
        let response = timeout_internal_grpc_call(self.client().get_loop_schedule(
            self.control_request(GetLoopScheduleRequest {
                registration_id: id.into(),
                after_firing_cursor: after,
                limit,
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.detail_json, "detail_json")
    }

    pub(crate) async fn revoke_loop_schedule(&self, id: &str) -> anyhow::Result<LoopRegistration> {
        let response = timeout_internal_grpc_call(self.client().revoke_loop_schedule(
            self.control_request(RevokeLoopScheduleRequest {
                registration_id: id.into(),
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.registration_json, "registration_json")
    }
}
