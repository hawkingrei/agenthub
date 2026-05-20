# 2026-05-21 Team Spec Refresh From External Daemon Review

## Summary

We reviewed a recent external daemon package and folded the stable operating concepts that still fit
AgentHub into the active Team specs. The result is a tighter contract around wakeup
ordering, reply-target fidelity, claim-before-deep-work routing, durable deferred follow-up, and
filesystem-backed recovery entry points. This pass also compacted the older MCP-era Team reference
docs so the active rules and the historical background no longer compete.

## Background

Earlier Team documentation already incorporated older external-daemon lessons around MCP-first
collaboration enforcement. The latest review showed that the more durable value is not one provider prompt
string, but a stronger operating contract:

- decide the visible reply/ownership surface early;
- keep reply routing on the original target/thread by default;
- distinguish quick-answer messages from durable execution work;
- use reminders/triggers instead of sleep-based waiting;
- treat runtime-injected identity and file-backed memory indexes as the recovery spine.

AgentHub already had mailbox, Team task, and workspace-memory specs, but these concepts were still
spread across several docs and skills. This pass moved them into the canonical feature specs and
reduced the older MCP-era spec to a short historical reference.

## Scope

Updated active specs:

- `docs/features/agents-teams.md`
- `docs/features/team-mailbox-intake-and-ownership.md`
- `docs/features/team-workspace-memory-contract.md`

Documentation-boundary cleanup:

- `docs/features/team-mcp-enforcement.md`
- `docs/features/README.md`
- `docs/README.md`
- `docs/journal/2026-03-05-team-mcp-enforcement-external-review.md`

## Key Decisions

1. Added a stable Team wakeup discipline contract.
   - Actors should handle runtime/review control input first, decide whether they owe an immediate
     visible acknowledgment or ownership signal, and only then spend context budget on deeper
     execution detail.

2. Made shared-visible communication surfaces explicit.
   - Local stdout, internal reasoning, and scratch files are not Team-visible communication.
   - Canonical visible updates remain conversation/channel replies, mailbox replies, thread replies,
     and canonical task notes.

3. Added reply-target fidelity and claim-before-deep-work rules.
   - Direct replies should stay on the original target/thread by default.
   - Quick-answer messages may resolve without spawning a task.
   - Durable execution work should claim mailbox ownership and/or attach a canonical Team task
     before extended work starts.

4. Mapped reminder-style follow-up into AgentHub terminology.
   - Deferred follow-up belongs on durable trigger/reminder paths with correlation metadata, not on
     sleep-based waiting or implicit memory recall.

5. Mapped daemon-style recovery expectations onto AgentHub workspace memory.
   - Runtime-injected identity is authoritative.
   - `AGENTS.md`, `TODO.md`, `.cache/context/state.md`, and worker `.agenthubmemory/` indexes form
     the stable recovery spine instead of ambient shell/process inference.

6. Compacted the old MCP-era references.
   - `docs/features/team-mcp-enforcement.md` now acts as a short historical mapping page instead of
     a parallel active spec.
   - docs guide files now state that historical specs/journals must point back to active canonical
     replacements instead of carrying new normative behavior.

## Validation

Reviewed the external daemon package metadata and prompt bundle, then reconciled the findings
against the current canonical Team specs:

```bash
sed -n '1,420p' docs/features/agents-teams.md
sed -n '1,320p' docs/features/team-mailbox-intake-and-ownership.md
sed -n '1,340p' docs/features/team-workspace-memory-contract.md
sed -n '1,240p' docs/features/team-mcp-enforcement.md
sed -n '1,220p' docs/features/README.md
sed -n '1,220p' docs/README.md
sed -n '1,220p' docs/journal/2026-03-05-team-mcp-enforcement-external-review.md
```

This was a docs-only change; no runtime tests were added in this PR.

## Follow-Ups

- Mailbox phase 3 implementation still needs the runtime/UI changes for richer thread/topic
  ownership and trigger-driven follow-up behavior.
- Prompt-tail slimming and reviewed Team memory continuity remain open follow-up work in
  `docs/todo.md`.
- Future external adapters (trigger/webhook/chat surfaces) should reuse the same reply-target,
  handling-disposition, and recovery-entry contracts rather than introducing separate semantics.
