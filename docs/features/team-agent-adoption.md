# Team Agent Adoption Specification

## Problem

The current Team `Add Agent` path is still centered on forging a brand-new Team-owned agent inside
the flow itself. That keeps Team setup dependent on agent creation even when the operator already
has suitable agents in the global AgentHub catalog.

AgentHub needs a first-class Team adoption model for existing agents, but the product semantics
must stay explicit:

- copying an existing agent into a Team is not the same as moving the original agent into that
  Team
- Team coordinator-first rules must still hold
- operator-facing UI should stay simple even when the underlying ownership/runtime rules differ

## Scope

- Team adoption of existing agents
- copy versus move semantics
- Team ownership, runtime, and history contracts for adopted agents
- UI expectations for `Add Existing Agent` versus `Create New Agent`

## Non-Goals

- Rewriting Team runtime execution semantics
- Allowing one live agent identity to belong to multiple Teams simultaneously
- Defining a full lineage/clone graph UI for copied agents
- Solving multi-operator governance for agent ownership transfer
- Replacing the existing `Create New Agent` Team forge flow in the same change

## Architecture

### 1) Two Explicit Adoption Modes

Team adoption should distinguish two separate flows:

1. `copy existing agent into Team`
2. `move existing agent to Team`

These are different product contracts and should not be collapsed into one vague "import agent"
surface.

### 2) Copy Mode

`copy` creates a new Team-owned agent using an existing global agent as the source template.

Expected behavior:

- source agent remains unchanged in the global agent catalog
- Team gets a new Team-owned member agent
- the new Team-owned member receives a new agent identity
- runtime continuity, ACP history, audit identity, and execution history are not inherited from
  the source agent

What may be copied:

- runtime command / args / preset shape
- workdir and worktree defaults
- code mode
- optional baseline description or prompt defaults

What should not be copied:

- source agent id
- active runtime session
- execution history
- workspace file contents
- workspace-local memory such as `.cache/context` and `.agenthubmemory`
- Team membership from the source side

### 3) Move Mode

`move` transfers an existing global agent into Team ownership.

Expected behavior:

- the original agent becomes the Team member
- Team membership uses the existing agent identity
- ownership scope changes from general/global agent management into Team ownership

This path is more powerful, but it is also riskier because it touches:

- route visibility
- runtime continuity
- audit/history ownership
- reversibility expectations

### 4) Recommended Delivery Order

The product should land in this order:

1. `copy` first
2. explicit copy extensions later
3. `move` later

Reasoning:

- `copy` is safer and easier to explain
- `copy` does not mutate the source agent
- workspace/content and memory/context copy need independent operator consent because they carry
  different data and provenance risks than configuration copy
- `move` needs stricter guardrails around running sessions, pending work, and ownership transfer

### 5) Coordinator Compatibility

Adoption must preserve the existing Team coordinator contract:

- an empty Team adopts its first agent as `coordinator`
- a Team that already has a coordinator adopts later agents as `worker`
- the default adoption flow should not reintroduce a free role toggle

## Contracts

### 1) Identity Contract

#### Copy

- Team member identity is the new Team-owned agent id
- source agent id remains unchanged
- Team spec references only the new Team-owned agent id

#### Move

- Team member identity is the existing agent id
- no second agent record is created
- the original standalone ownership is replaced by Team ownership

### 2) Ownership Contract

#### Copy

- source agent remains visible in the global agent catalog
- copied Team member becomes Team-owned only

#### Move

- moved agent becomes Team-owned
- global standalone management should no longer present it as an ordinary unattached agent by
  default

### 3) Runtime Contract

#### Copy

- copied Team member does not inherit the source runtime session
- Team runtime starts and manages the copied member under Team rules
- the copied Team member inherits workspace path/worktree configuration only
- the copied Team member does not clone the source workspace contents as part of the default flow

#### Move

The first move rollout should be conservative:

- allow move only for stopped agents
- reject agents with active Team/runtime execution dependencies
- either rebind or restart runtime under Team ownership instead of pretending that the old session
  is unchanged

### 4) History Contract

#### Copy

- source execution history remains on the source agent only
- copied Team member starts with its own Team-scoped history
- source filesystem memory and workspace-local context do not carry over into the copied Team
  member by default

#### Move

- existing agent history remains attached to the moved agent identity
- Team UI should treat post-move activity as Team-owned
- whether pre-Team history appears inside the default Team flow should remain a later decision;
  first rollout should prefer a conservative visibility boundary

### 5) UI Contract

`Add Agent` should support two top-level choices:

1. `Add Existing Agent`
2. `Create New Agent`

Within `Add Existing Agent`, the source agent list should support:

- `Copy into Team`
- `Move to Team`

If `move` is not yet supported or not valid for a given agent, the UI should:

- disable or hide `Move to Team`
- explain why in plain language

### 6) First-Rollout Contract

The first implementation slice should only require:

- `Add Existing Agent`
- `Copy into Team`
- Team coordinator-first adoption semantics

Current status:

- `copy existing agent into Team` is the active implementation path
- `move existing agent to Team` remains specified but deferred
- default `copy` currently copies agent configuration only:
  - workspace path and worktree settings may carry over
  - workspace contents, runtime history, and memory do not

`Move to Team` should remain specified but not required for the first delivery slice.

### 7) Extension Mode Contract

After the copy-first rollout, adoption extensions must stay separate reviewable modes. They are not
implicit side effects of `Copy into Team`.

#### Stopped-Only Move

`Move existing agent to Team` transfers the original agent identity into Team ownership. The first
move rollout should be stopped-only:

- reject agents with an active runtime session
- reject agents with active Team membership, pending permission reviews, or running task ownership
  dependencies
- require an explicit operator action that names the target Team
- keep pre-move execution history attached to the same agent identity
- start post-move runtime under Team ownership instead of pretending an old standalone session was
  already Team-managed

The moved agent should stop appearing as a normal unattached global agent by default once the
ownership transfer succeeds.

#### Workspace-Content Copy

Workspace-content copy is an optional extension to configuration copy. It should clone a source
workspace snapshot into the new Team-owned member workspace without changing the source agent.

This mode must:

- be opt-in and distinct from default configuration copy
- require an explicit destination workspace or generated destination workspace path
- reject ambiguous self-copy cases where source and destination paths resolve to the same workspace
- record source provenance for audit and troubleshooting
- define collision behavior before copying files into an existing destination
- avoid copying ignored build/cache artifacts unless a later contract explicitly allows them

Workspace-content copy still creates a new Team-owned agent identity. It is not a move.

#### Memory And Context Seeding

Memory/context seeding is a narrower extension than workspace-content copy. It may seed selected
workspace-local context into the new Team member without copying the whole filesystem.

This mode must:

- be opt-in and separately named from workspace-content copy
- define which context sources are eligible before implementation
- filter or reject secrets and machine-local cache state
- record provenance and seed timestamp on the new Team-owned member
- never link the new member to mutable source-agent memory in a way that can leak future updates

Seeded memory/context should be a snapshot input for the copied Team member, not shared runtime
state.

## Validation Matrix

### Copy First Rollout

- focused web tests for:
  - listing eligible existing agents
  - copying the first adopted agent into an empty Team as coordinator
  - copying later adopted agents into a Team with an existing coordinator as workers
  - leaving the source agent unchanged after copy
- focused backend tests for:
  - creating a new Team-owned agent record from a source agent template
  - preserving the source agent identity and ownership
  - writing the correct Team spec member entries
- browser-level integration coverage for:
  - create Team shell
  - add existing agent via copy as first coordinator
  - add another existing agent via copy as worker

### Move Later Rollout

- focused tests for:
  - stopped-only move guard
  - active-runtime, pending permission-review, and running task dependency rejection
  - ownership transfer
  - Team visibility after move
  - post-move runtime start under Team ownership

### Later Copy Enhancements

- focused tests for any future opt-in copy extensions such as:
  - workspace-content clone
  - memory/context seeding
  - explicit source-provenance display
  - destination workspace collision handling
  - secret/cache filtering for seeded context

## Operational Notes

- Prefer `copy` as the default safe recommendation until `move` has clear runtime/history
  constraints.
- Keep adoption copy concise in UI terms: operators should understand whether they are cloning a
  template or transferring ownership.
- Do not silently expand default `copy` into workspace snapshot or memory cloning. Any later
  support for those behaviors should be explicit, reviewable, and separately validated.
- Treat stopped-only move, workspace-content copy, and memory/context seeding as separate product
  modes even if they share some implementation helpers.
- Avoid mixing adoption semantics with unrelated Team runtime or multi-user administration concepts.

## Open Risks

- `move` can blur the line between standalone agents and Team-owned members if visibility rules are
  not explicit
- `copy` can increase agent count and naming pressure unless copied members retain clear source
  provenance metadata
- later workspace-copy or memory-copy support could easily blur `copy` versus `move` semantics if
  introduced without explicit opt-in controls
- runtime continuity for moved agents can become misleading if running sessions are silently
  preserved without a Team-specific rebinding contract
- future "move back out of Team" behavior will need an explicit reversibility contract

## Source Journals

- `docs/journal/2026-05-03-team-agent-adoption-contract.md`
- `docs/journal/2026-05-03-team-add-existing-agent-copy.md`
- `docs/journal/2026-05-06-team-adoption-move-deferred.md`
- `docs/journal/2026-07-19-team-adoption-extension-contract.md`
