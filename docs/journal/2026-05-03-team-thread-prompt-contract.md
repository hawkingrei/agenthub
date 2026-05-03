# Team Thread Prompt Contract

## Summary

- tightened the default Team coordinator and worker prompts so `channel root = summary entrypoint`
  and `thread = full context container` are explicit runtime guidance instead of spec-only intent
- updated the Team thread pane helper copy so the UI reinforces the same summary-first versus
  full-context split
- updated the main Team channel composer helper copy so new root posts are explicitly framed as
  summary-first and thread-oriented follow-up is signposted before sending
- tightened the default Team prompts again so agents are told to proactively use
  `agenthub actor team-thread-open` and `agenthub actor team-thread-reply` when a channel summary
  needs a deeper thread-backed context
- documented the current concurrency boundary: passive root readers are not auto-enrolled into a
  thread that someone else opens later, so automatic thread forwarding remains participant- and
  mention-based

## Background

- `docs/features/team-channels-threads.md` already defined the channel/thread split as part of the
  Team context-budget model
- the shipped default Team prompts still focused on routing and ownership, but they did not state
  clearly enough that deeper background, logs, and evidence should move into the thread by default
- the thread pane helper copy only described anchoring behavior, which left the product meaning of
  the split implicit

## Scope

- default Team coordinator prompt contract
- default Team worker prompt contract
- thread-pane helper copy
- channel-composer helper copy
- agent-side thread-open and thread-reply usage guidance
- thread participant forwarding boundary for late-opened threads
- focused Rust/API/web regression coverage for the new contract lines

## Key Decisions

- keep the slice narrow: prompt defaults and visible helper copy changed, but backend thread
  storage/routing behavior did not
- encode the same rule in both runtime prompts and the visible thread pane:
  - channel root stays summary-first
  - thread carries detailed context, logs, evidence, and follow-up
- lock the change through both prompt-crate tests and Team API tests so future prompt edits cannot
  silently drop the thread-context contract

## Validation

```bash
cargo test -p agenthub-team-prompts
cargo test teams_api_injects_role_workflow_prompt_policy_defaults -- --nocapture
cargo test actor_help_for_team_thread_topics_describes_summary_first_flow_and_participants -- --nocapture
cd web && npm exec vitest -- run src/pages/team_panels.test.tsx src/pages/team/team_thread_pane.test.tsx src/pages/team/use_team_workspace_view_model.test.tsx
cd web && npm exec tsc -- --noEmit
```

## Follow-Ups

- continue the broader `P0+` channel/thread rollout with runtime behavior and UI slices that move
  more topic-specific back-and-forth out of the shared root channel
