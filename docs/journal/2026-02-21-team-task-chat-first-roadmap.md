# Team Main Task Chat-First Roadmap

## Background

Current Team workbench interaction still exposes run-centric controls in the primary surface.  
Product direction is to make Team collaboration feel like a real group chat:

- User talks to Team (leader/member/group) first.
- Leader drives planning/negotiation before execution.
- `Run` becomes an execution artifact, not the first user-facing concept.

This note captures the phased rollout plan and verification targets for that direction.

## Goals

- Shift Team UX from run-first to conversation-first.
- Introduce persistent task and conversation artifacts for replay/audit/handoff.
- Keep deterministic execution by compiling negotiated task context into structured run specs.
- Improve inter-agent discoverability with capability cards.

## Non-Goals

- Replacing existing debug run operations in one step.
- Rewriting the entire Team orchestrator pipeline in a single PR.
- Changing existing role policy (leader vs worker startup constraints).

## Target Experience

1. User opens Team and starts a task by chatting with leader/team.
2. Leader negotiates scope and records task list / acceptance criteria.
3. System compiles the agreed scope into deterministic run spec (`spec.steps` template).
4. Workers execute; leader synthesizes and returns final deliverable.
5. All conversation/task artifacts are replayable; secrets/tokens are never stored in DB.

## Phased Implementation Plan

### Phase 1: Conversation-First Surface

- Hide `Create Run` from primary Team surface (retain under Debug fallback).
- Promote Team chat entry as the primary kickoff action.
- Keep existing run ops callable for compatibility.

### Phase 2: Main Task + Conversation Persistence

- Add explicit models/tables for:
  - task metadata
  - conversation messages/events
- Persist full dialogue/task artifacts for replay/audit.
- Enforce storage redaction policy for provider tokens and sensitive runtime fields.

### Phase 3: Leader Plan Compiler

- Define deterministic translation from negotiated conversation to run payload:
  - fixed step template
  - role-bound assignee mapping
  - acceptance criteria + deadline fields
- Add preview/confirm step before execution start.

### Phase 4: User-As-Actor + Routing Modes

- Model user as a special actor in Team message semantics.
- Support routing modes:
  - `to_leader`
  - `to_member`
  - `group_chat`
- Keep globally ordered replay for mixed user/agent traffic.

### Phase 5: Agent Discovery Card

- Expose capability metadata through a well-known discovery endpoint.
- Render card metadata in Team UI before peer-to-peer delegation.
- Align with mailbox/orchestrator routing contracts.

## Validation Plan

- API tests:
  - task lifecycle CRUD and replay semantics
  - conversation persistence and redaction behavior
  - compile-to-run payload schema checks
- UI tests:
  - primary chat-first kickoff behavior
  - routing-mode correctness and message ordering
  - discovery card rendering + fallback behavior
- E2E tests:
  - `task -> negotiation -> compile run -> execute -> synthesize` golden path
  - debug run-ops regression path unchanged

## Risks and Mitigations

- **Risk:** Mixed old/new entry paths confuse users.
  - **Mitigation:** Keep Debug fallback explicit and add UI hints during migration.
- **Risk:** Conversation payloads accidentally persist secrets.
  - **Mitigation:** Add explicit redaction gate and storage-level tests.
- **Risk:** Compile stage introduces nondeterministic execution.
  - **Mitigation:** Keep fixed step template and validate with schema/unit tests.

