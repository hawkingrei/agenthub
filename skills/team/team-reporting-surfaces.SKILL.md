---
name: team-reporting-surfaces
description: Use when deciding whether an update belongs in task notes, mailbox, or channels.
---

# Team Reporting Surfaces

Use this skill when a Team agent must decide how to make local execution state visible to other
agents or humans.

This skill defines:

- why direct local output is not a shared Team surface
- when durable task state belongs in task notes
- when directed coordination belongs in mailbox
- when broader visibility belongs in channel roots or threads

Shared routing vocabulary remains canonical in `skills/team/AGENTS.md`. Mailbox transport details
remain canonical in `team-actor-mailbox.SKILL.md`.

## Core Visibility Rule

- Direct local output, scratchpad text, and stdout/stderr are not shared Team surfaces.
- A fact is not reliably visible to teammates or humans unless it is routed through a shared
  surface.
- Do not assume a local command result, tool output, or one-agent reply is visible to other
  participants.
- If the current inbound item already has a concrete reply target or thread target, treat that
  surface as the default place to make the next visible update unless you intentionally escalate the
  audience.

## Surface Selection

Use `task note` when:

- canonical task state changed
- a durable task TODO, blocker note, decision, or evidence summary should survive the current turn
- reassignment, reprioritization, or context meaning changed and the reason should stay attached to
  the task

Use `mailbox` when:

- exactly one teammate or the coordinator needs the update
- the message is a blocker escalation, dependency handoff, scoped review request, or targeted
  execution checkpoint
- the information matters now but does not need broad team visibility

Use `shared-channel` or thread when:

- the update changes shared plans, risks, dependencies, review state, or human-visible progress
- a human asked in-channel and a visible acknowledgement or answer should land back in that same
  shared surface
- multiple teammates need to see the update at once

Use a direct visible reply with no new task when:

- the question can be answered immediately from current facts
- no durable ownership, multi-step execution, or lifecycle tracking is needed beyond that reply

## Preferred Order

For durable execution updates, prefer this order:

1. write task note for canonical task state
2. send mailbox for directed coordination
3. post channel root or thread update for broad visibility

These surfaces are complementary, not exclusive. One fact may need both a durable task note and a
short visible channel summary.

## Channel And Thread Split

- Keep channel root messages summary-first.
- Use threads for deeper context: detailed evidence, logs, reasoning, and follow-up discussion.
- If a new topic needs deeper context after a summary root lands, open or continue a thread instead
  of turning the root channel lane into a context dump.

## Guardrails

- Do not let important execution evidence exist only in private local output.
- Do not broadcast routine one-peer coordination into shared channels when mailbox is enough.
- Do not use mailbox as the only record for canonical task-state changes when a task note should
  exist.
