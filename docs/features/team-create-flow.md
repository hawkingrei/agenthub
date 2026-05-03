# Team Create Flow Specification

## Problem

The current Team creation surface still carries too much staged ceremony around coordinator and
member setup, while also overreaching on when coordinator choice needs to happen.

That creates three product problems:

- `Create Team` risks becoming a heavier wizard than the current single-operator product path
  needs
- the `Add Agents` surface still exposes more process than value
- the operator still does not get a crisp product signal that the first added agent becomes the
  coordinator

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

`Create Team` should stay lightweight and create the Team shell first.

The normal flow should feel like:

1. define the mission
2. create the Team shell
3. add the first participating agent
4. make it explicit that this first agent becomes the coordinator
5. add more workers afterward as needed

It should not feel like:

1. create a team shell
2. guess whether coordinator selection is still pending
3. discover role semantics only after entering a larger forge flow
4. later reconcile the result

### 2) First-Agent Coordinator Contract

Required product meaning:

- every Team that has members must still end up with exactly one coordinator
- the coordinator does not need to be chosen during the initial `Create Team` modal
- the first added Team agent becomes the coordinator by default
- there should be no ambiguity about that first-agent contract in the add-agent flow

Operational consequence:

- `Create Team` should collect only mission and Team identity fields
- the first `Add Agent` path should make it obvious that the first added member becomes the
  coordinator
- worker role selection remains unavailable until a coordinator already exists

### 3) Add Agents Simplification Contract

`Add Agents` should become a thin selection step, not a mini control plane.

Required direction:

- show a compact list/picker of available agents
- allow selecting which agents participate in the Team
- make the first added agent the coordinator by default
- assign all later added members to `worker` by default unless the Team is still empty

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
- first added agent becomes coordinator
- zero or more workers added afterward

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
- `Team shell`
  - create the Team without members yet
- `First agent`
  - add the planning/coordinator agent
- `Additional agents`
  - add workers as needed

Manual-spec entry can remain available for advanced users, but it should not distort the normal
guided flow.

### 6) Validation Rules

The create flow should enforce:

- mission details must be present before Team creation:
  - Team name
  - mission / description
- exactly one coordinator once the Team has members
- the first added member must be the coordinator
- workers cannot be added before a coordinator exists
- selected members must have stable `member_id` inputs for Team spec generation

### 7) UI Simplification Rules

Required UI direction:

- fewer stages
- fewer empty states
- fewer explanatory banners
- fewer advanced controls visible by default
- one clear first-agent hint instead of role-management ceremony

Explicit anti-goals:

- do not keep both old forge-stage language and the new simplified model in the same default flow
- do not make the user revisit role assignment after already choosing participants
- do not add more setup chrome while trying to simplify the page

## Contracts

### 1) Team Spec Contract

- the created Team spec must always contain exactly one `coordinator_member_id` once
  `spec.members[]` is non-empty
- the member referenced by `coordinator_member_id` must also appear in `spec.members[]`
- the first added member becomes coordinator
- all later non-coordinator selected members default to `worker`

### 2) UI Contract

- the normal create flow creates the Team shell first
- the first `Add Agent` interaction explicitly explains that this member becomes coordinator
- `Add Agents` remains a compact participant-selection surface, not a role-management wizard
- the simplified flow is the default path; advanced/manual spec remains secondary

### 3) Product Boundary Contract

- current Team create UX assumes one operator and current in-product agents
- user/device management is not part of the Team create path
- the flow should stay aligned with the current single-coordinator Team model

## Validation Matrix

- focused Team create component tests for:
  - `Create Team` stays mission-only
  - first added agent is clearly presented as coordinator
  - selected workers stay unavailable until coordinator exists
- browser-level integration coverage for:
  - create Team on small screens
  - create Team shell, then add first coordinator agent
  - add later workers without any separate coordinator-promotion step

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
- the product may drift back into "coordinator chosen at team creation" language unless the create
  shell and add-first-agent boundaries stay explicit

## Source Journals

- `docs/journal/2026-02-19-team-create-dual-entry-modes.md`
- `docs/journal/2026-02-19-team-create-wizard-manual-spec-flow.md`
