## Summary

Extended the default Team ACP permission-review human fallback delay from 45 seconds to 5 minutes, and changed the shared `all` fallback from a plain text notice into a structured permission review card.

## Why

The previous 45-second fallback was too aggressive for real review flows. It caused human-review escalation messages to appear in the shared `all` channel while the assigned reviewer still had a reasonable chance to handle the permission request.

The old fallback message also pointed users at a separate Permissions panel, which made the shared channel feel disconnected from the action that required approval.

## What changed

- `TeamPermissionReviewDispatcherSettings::default().human_fallback_delay` now defaults to `Duration::from_secs(300)`.
- Human-review fallback messages in the shared `all` channel now use a structured `permission_review_card` payload instead of a plain `chat_message`.
- Team channel rendering now shows inline permission action buttons directly in the shared conversation.
- After a human responds, the card collapses to a status-only surface instead of continuing to show the action buttons.
- Added a unit test locking the default to 5 minutes.

## Validation

- `cargo test permission_review_dispatcher_default_human_fallback_is_five_minutes -- --nocapture`
- `cargo test dispatches_worker_permission_to_leader_and_can_fallback_to_human_review -- --nocapture`
- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- `make build-web`
