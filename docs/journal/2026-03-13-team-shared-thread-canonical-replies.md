## Summary

Team shared-thread agent replies no longer depend on frontend mailbox merge logic.
Mailbox `chat_message` replies that target the human actor are now persisted server-side as
canonical `team_conversation_messages` rows in the shared `all` thread.

## Why

The previous `/teams` conversation UI reconstructed human-visible agent replies by merging
delivered mailbox messages into the shared thread in the browser.

That had three problems:

1. The canonical conversation record was incomplete.
2. The frontend owned reply reconstruction semantics that belong on the server.
3. Replay and future multi-thread/channel behavior would inherit a fragile read-model hack.

## What Changed

### Server-side canonical persistence

- `src/team/manager/mailbox.rs`
  - Added canonical shared-thread persistence inside the mailbox transaction.
  - Human-visible local mailbox replies now:
    - resolve or create the shared `all` thread for the run's team;
    - persist a canonical `team_conversation_messages` row with route `group_chat`;
    - store only final chat payload fields (`type`, `text`, optional `correlation_id`);
    - drop internal transport/status fields such as `current_phase`.
- `src/team/manager.rs`
  - Exposed `redact_sensitive_json` to the mailbox module for canonical payload storage.

### Shared thread bootstrap

- Shared thread lookup now prefers:
  - task title `all`; or
  - `context_json.bootstrap_kind == "shared_thread"`.
- If no shared thread exists yet, the server bootstraps one during canonical reply persistence.

### Frontend read-model simplification

- `web/src/pages/team_task_panel.tsx`
  - Removed mailbox merge behavior from the shared thread.
  - Shared thread now renders only canonical conversation records.
  - Agent replies already persisted in the shared thread appear like normal conversation messages.
- `web/src/pages/team_page.tsx`
  - Removed the mailbox-message prop wiring into `TeamTaskPanel`.

### Tests

- `src/team/manager/tests.rs`
  - Added:
    - `actor_mailbox_service_persists_agent_reply_into_shared_thread`
    - `actor_mailbox_service_deduped_shared_thread_reply_does_not_duplicate_conversation`
    - `actor_mailbox_service_does_not_persist_agent_to_agent_chat_into_shared_thread`
    - `actor_mailbox_service_canonicalizes_stringified_json_reply_into_shared_thread`
    - `actor_mailbox_service_reuses_existing_shared_thread_for_canonical_reply`
- `web/src/pages/team_panels.test.tsx`
  - Updated shared-thread tests to assert canonical conversation rendering rather than mailbox merge.

## Validation

Local validation completed:

- `cargo test actor_mailbox_service_persists_agent_reply_into_shared_thread -- --nocapture`
- `cargo test actor_mailbox_service_deduped_shared_thread_reply_does_not_duplicate_conversation -- --nocapture`
- `cargo test actor_mailbox_service_does_not_persist_agent_to_agent_chat_into_shared_thread -- --nocapture`
- `cargo test actor_mailbox_service_canonicalizes_stringified_json_reply_into_shared_thread -- --nocapture`
- `cargo test actor_mailbox_service_reuses_existing_shared_thread_for_canonical_reply -- --nocapture`
- `cd web && npx vitest run src/pages/team_panels.test.tsx --pool=threads --maxWorkers=1`

Follow-up verification still required after push/PR:

- verify deployed shared-thread reply behavior on `/teams`;
- verify both directed human replies and broadcast replies preserve correct canonical routing;
- record push + PR CI run IDs.
