# 2026-05-21 Team Prompt Operating Contract Refresh

## Summary

Refreshed the Team coordinator/worker prompt text and the related Team skills so the runtime-facing
instructions more closely match the operating contract we want agents to follow in practice.

## Background

Recent Team spec cleanup made the desired operating model clearer: agents should decide visible
reply/ownership obligations early, stay on the original reply target by default, distinguish quick
answers from durable execution work, and treat filesystem-backed recovery pointers as the stable
re-entry spine.

The prompt and skill text already covered parts of this, but the rules were not yet explicit enough
to feel like a reliable operating contract.

## Scope

- `crates/agenthub-team-prompts/prompts/default_team_coordinator_prompt.txt`
- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
- `crates/agenthub-team-prompts/src/lib.rs`
- `skills/team/AGENTS.md`
- `skills/team/team-coordinator-orchestrator.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`
- `skills/team/team-actor-mailbox.SKILL.md`
- `skills/team/team-reporting-surfaces.SKILL.md`

## Key Decisions

1. Added early visible-acknowledgement / blocker / ownership guidance.
   - Agents should decide whether they owe a visible acknowledgment before spending time on deeper
     execution or context gathering.

2. Added reply-target fidelity guidance.
   - If an inbound item already has a reply target or thread target, the default visible answer
     should stay there instead of opening a parallel lane.

3. Made quick-answer vs durable-execution routing more explicit.
   - Quick factual answers can stay as direct visible replies.
   - Durable multi-step work should still claim mailbox/task ownership.

4. Added recovery-spine guidance.
   - Runtime metadata plus `AGENTS.md`, `TODO.md`, `.cache/context/state.md`, and worker
     `.agenthubmemory/` should be treated as the authoritative recovery spine instead of ambient
     shell inference.

5. Added pause/handoff guardrails.
   - Before pausing on unfinished work, agents should emit the minimal blocker or handoff update
     needed for cheap recovery by the next owner.

## Validation

```bash
cargo test -p agenthub-team-prompts
cargo test -p agenthub-managed-skills -- --nocapture
```

## Follow-Ups

- Team mailbox phase 3 still needs the runtime/UI side of richer wakeup, reminder, and ownership
  behavior.
- Prompt-tail slimming and Team memory continuity remain open follow-up tracks in `docs/todo.md`.
