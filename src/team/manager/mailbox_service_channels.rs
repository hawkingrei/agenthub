use serde_json::Value;

use super::mailbox::{SendActorMessageInput, build_channel_mailbox_forward_payload};
use super::mailbox_payloads::{
    channel_payload_correlation_id, ensure_channel_message_correlation_id,
    normalize_channel_message_payload,
};
use super::mailbox_service::TeamActorMailboxService;
use crate::team::TeamActorMessageTransport;
use agenthub_team_actor::{ActorMessageKind, ActorSendResponse};

impl TeamActorMailboxService {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn send_channel_message(
        &self,
        run_id: &str,
        from_actor_id: &str,
        from_peer_id: &str,
        channel_id: &str,
        channel: &str,
        message_kind: Option<ActorMessageKind>,
        payload: Value,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<ActorSendResponse> {
        let target = self
            .manager
            .resolve_channel_mailbox_target(run_id, channel_id, from_actor_id)
            .await?;
        if target.recipient_actor_ids.is_empty() {
            anyhow::bail!("channel '{}' has no recipient agents", channel_id);
        }
        let recipient_deliveries = self
            .manager
            .resolve_channel_recipient_deliveries(&target.recipient_actor_ids)
            .await?;

        let mention_actor_ids = self
            .manager
            .extract_channel_mention_actor_ids(run_id, &payload)
            .await?;
        let normalized_payload = normalize_channel_message_payload(payload);

        let base_idempotency_key = idempotency_key.map(str::to_string).unwrap_or_else(|| {
            agenthub_team_actor::build_default_actor_channel_idempotency_key(
                run_id,
                from_actor_id,
                from_peer_id,
                channel_id,
                channel,
                TeamActorMessageTransport::Local.as_str(),
                None,
                &normalized_payload,
            )
        });
        let canonical_payload = ensure_channel_message_correlation_id(
            normalized_payload,
            Some(base_idempotency_key.as_str()),
        );
        let (authority_message_id, source_payload) = if let Some(existing) = self
            .manager
            .find_channel_message_by_correlation_id(
                &target.conversation_id,
                from_actor_id,
                channel_payload_correlation_id(&canonical_payload)
                    .expect("canonical channel payload should carry correlation_id"),
            )
            .await?
        {
            (existing.message_id, existing.payload)
        } else {
            let canonical_message = self
                .manager
                .append_task_conversation_message(
                    &target.task_id,
                    from_actor_id,
                    None,
                    "group_chat",
                    canonical_payload.clone(),
                )
                .await?;
            (canonical_message.message_id, canonical_payload.clone())
        };

        let mut first_result = None;
        let mut any_created = false;
        for delivery in &recipient_deliveries {
            let forwarded_payload = build_channel_mailbox_forward_payload(
                &source_payload,
                &target,
                channel_id,
                authority_message_id,
                mention_actor_ids.as_slice(),
            );
            let fanout_idempotency_key =
                agenthub_team_actor::build_actor_channel_fanout_idempotency_key(
                    &base_idempotency_key,
                    delivery.actor_id.as_str(),
                );
            let result = self
                .manager
                .send_actor_message_with_created_kind(
                    SendActorMessageInput {
                        run_id,
                        from_actor_id,
                        from_peer_id,
                        to_actor_id: delivery.actor_id.as_str(),
                        to_peer_id: delivery.to_peer_id.as_str(),
                        channel,
                        transport: delivery.transport.clone(),
                        route: delivery.route.clone(),
                        payload: forwarded_payload,
                        message_kind: None,
                        idempotency_key: Some(fanout_idempotency_key.as_str()),
                    },
                    message_kind.clone(),
                )
                .await?;
            any_created |= result.1;
            if first_result.is_none() {
                first_result = Some(result);
            }
        }

        let (message, created) = first_result.expect("channel fanout should produce a message");
        Ok(ActorSendResponse {
            message_id: message.message_id,
            state: message.status.clone(),
            deduped: !created && !any_created,
            created_at: message.created_at,
            message,
        })
    }
}
