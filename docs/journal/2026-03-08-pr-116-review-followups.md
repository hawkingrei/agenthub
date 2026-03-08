# 2026-03-08 PR #116 Review Follow-ups

## Context

PR #116 still had several unresolved review threads after the CI fixes landed. The remaining
comments fell into three buckets:

- low-risk correctness issues in the Teams workbench read model;
- small maintainability issues in the local developer-mode and sidebar surfaces;
- one ACP defensive-runtime issue plus one larger ACP fairness concern.

This follow-up intentionally fixes the correctness and maintainability items in-place, applies the
ACP defensive fix, lands the minimal mutation-barrier fairness fix for concurrent Codex prompts,
and trims one remaining tracing hot-path allocation.

## Changes

- hardened browser-local developer-mode preference parsing to reject array payloads instead of
  treating them as valid preference objects;
- stopped exposing raw team IDs through sidebar hover titles when developer mode is disabled;
- simplified sidebar copy by removing the dead `Browse runs` conditional and dropping the static
  `Channels 1` label down to `Channels`;
- removed dead `TeamTaskPanel` props (`tasks`, `selectedTaskId`, `onSelectedTaskIdChange`) after
  the shared-thread selector UI was removed;
- changed shared-thread resolution to prefer the explicit `all`/`shared_thread` task instead of
  falling back to the most recently active task, and aligned the Teams workspace title/messages
  with that resolved shared-thread target;
- fixed the Bazel-sensitive Teams runtime-env pagination test to advance with the oldest returned
  event id instead of the newest one;
- changed the ACP session loop to abort when the prompt-completion channel closes unexpectedly
  rather than forcing `active_prompt_count` to zero and releasing queued session mutations
  unsafely.
- added a mutation barrier to concurrent Codex ACP prompt delivery so once a session mutation is
  queued, later prompts queue behind it instead of overtaking it indefinitely;
- changed `agenthub-codex-acp`'s tracing visitor to capture the `message` field only once, which
  avoids redundant string allocations on later `record_str` visits in the same event.

## Deferred Follow-up

- `AllowConcurrentPrompts` still has no explicit max-concurrency cap, so a separate backpressure
  limit may still be desirable if prompt fan-in grows large enough to stress the runtime. That
  follow-up remains tracked in `docs/todo.md`.

## Validation

- `cargo test -p agenthub-acp prompt_delivery_policy_is_provider_aware -- --nocapture`
- `cargo test start_agent_with_actor_context_injects_runtime_env_vars -- --nocapture`
- `cd web && npm run lint`
- `cd web && npx vitest run src/ui/developer_mode.test.ts src/pages/team/page_helpers.test.ts src/pages/team_panels.test.tsx --pool=threads --maxWorkers=1`
- `make build-web`
