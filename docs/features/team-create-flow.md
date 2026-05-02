# Team Create Flow Specification

## Problem

The current Team creation surface still carries too much staged ceremony around coordinator and
member setup.

That creates three product problems:

- the operator still feels like they need to "add a leader/coordinator later" instead of deciding
  that role at creation time
- the `Add Agents` surface exposes more process than value
- the current flow reflects older forge-stage assumptions more than the actual current product
  boundary

AgentHub does not currently position multi-user administration as the primary Team setup model.
The Team creation flow should therefore optimize for one operator creating a team quickly and
explicitly.

## Scope

- Team creation information architecture
- coordinator selection timing
- `Add Agents` simplification direction
- alignment with the single-coordinator Team contract

## Non-Goals

- Rewriting Team runtime semantics
- Replacing the underlying Team spec structure
- Reintroducing multi-user or device-management complexity into Team setup
- Defining every low-level wizard component state

## Architecture

### 1) Core Principle

Team creation should decide the coordinator up front.

The creation flow should feel like:

1. define the mission
2. choose the participating agents
3. choose which one is the coordinator
4. launch

It should not feel like:

1. create a team shell
2. later add a coordinator
3. later reconcile roles
4. later simplify the result

### 2) Coordinator-First Creation Contract

Required product meaning:

- every created Team must have exactly one coordinator
- coordinator selection should happen during Team creation itself
- there should be no later "promote to coordinator" or separate coordinator-forge stage required
  for the normal flow

Operational consequence:

- `Create Team` should collect `coordinator_member_id` as part of the normal creation form
- the UI should make it obvious that one selected member is the coordinator and the rest are
  workers
- role selection should be explicit, but the interaction should stay lightweight

### 3) Add Agents Simplification Contract

`Add Agents` should become a thin selection step, not a mini control plane.

Required direction:

- show a compact list/picker of available agents
- allow selecting which agents participate in the Team
- allow one inline coordinator choice among the selected set
- assign all other selected members to `worker` by default

The first simplified flow should avoid:

- multi-stage role ceremony
- separate "add leader" vs "recruit workers" mental models
- thick configuration forms before the operator has even chosen the participants
- exposing unrelated admin concepts such as devices or multi-user onboarding in the Team create
  path

### 4) Single-Operator Product Boundary

Current Team creation should optimize for the current product boundary:

- one logged-in operator
- one set of available agents
- one coordinator selected at creation time
- zero or more workers selected at creation time

This means the create flow should prefer:

- directness
- fewer stages
- less explanation text
- less conditional branching

It should not assume:

- user/device provisioning as part of Team creation
- role negotiation across multiple human operators
- a later admin-only cleanup pass to make the Team valid

### 5) Guided Flow Direction

Recommended first-class guided shape:

- `Mission`
  - Team name
  - short mission / description
- `Members`
  - pick participating agents
  - choose one as coordinator
- `Review`
  - show the resulting Team shape
  - launch

Manual-spec entry can remain available for advanced users, but it should not distort the normal
guided flow.

### 6) Validation Rules

The create flow should enforce:

- exactly one coordinator
- coordinator must be one of the selected members
- selected members must have stable `member_id` inputs for Team spec generation
- launching should fail early if the resulting Team would have zero members or no coordinator

### 7) UI Simplification Rules

Required UI direction:

- fewer stages
- fewer empty states
- fewer explanatory banners
- fewer advanced controls visible by default
- one clear ownership selector instead of role-management ceremony

Explicit anti-goals:

- do not keep both old forge-stage language and the new simplified model in the same default flow
- do not make the user revisit role assignment after already choosing participants
- do not add more setup chrome while trying to simplify the page

## Contracts

### 1) Team Spec Contract

- the created Team spec must always contain exactly one `coordinator_member_id`
- the member referenced by `coordinator_member_id` must also appear in `spec.members[]`
- all non-coordinator selected members default to `worker`

### 2) UI Contract

- the normal create flow chooses coordinator inline during creation
- `Add Agents` is a compact participant-selection surface, not a role-management wizard
- the simplified flow is the default path; advanced/manual spec remains secondary

### 3) Product Boundary Contract

- current Team create UX assumes one operator and current in-product agents
- user/device management is not part of the Team create path
- the flow should stay aligned with the current single-coordinator Team model

## Validation Matrix

- focused Team create component tests for:
  - coordinator must be selected at create time
  - exactly one coordinator
  - selected workers default correctly
- browser-level integration coverage for:
  - create Team on small screens
  - select agents and choose coordinator inline
  - launch without any later coordinator-promotion step

## Operational Notes

- Keep the default Team create path optimized for speed and clarity, not for exposing every
  historical forge-stage option.
- Manual spec entry may remain for advanced operators, but it should stay visually secondary and
  must not reintroduce complexity into the normal guided flow.

## Open Risks

- legacy forge wording may continue to leak into the simplified flow if old stages remain only
  partially removed
- manual-spec support can keep reintroducing advanced-state complexity into the default path unless
  the entry modes stay clearly separated

## Source Journals

- `docs/journal/2026-02-19-team-create-dual-entry-modes.md`
- `docs/journal/2026-02-19-team-create-wizard-manual-spec-flow.md`
