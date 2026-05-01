# Unified Workspace Information Architecture Specification

## Problem

AgentHub currently exposes `Agents` and `Teams` as separate top-level product surfaces.

This preserves domain boundaries, but it also creates product drift:

- operators switch between two different mental models for closely related work;
- shared cross-cutting views such as channels, tasks, members, and search do not have one canonical
  home;
- `Agent` objects feel runtime-centric while `Team` objects feel collaboration-centric, even though
  both live inside the same workspace boundary;
- recent Team UI work intentionally moved toward a compact Notion-like style, while the existing
  split makes it harder to present one coherent workbench shell.

We want one unified workspace shell that:

- preserves AgentHub-specific Team semantics (`conversation`, `task`, `run`, `step`, mailbox,
  actor runtime);
- preserves Agent-specific execution/workspace identity (`workspace`, ACP, local runtime,
  node/workdir affinity);
- learns from Slock-style object-centric navigation without copying its product model directly;
- keeps the current Notion-style content-first visual direction instead of regressing into a
  Slack-like control-heavy shell.

## Scope

- Top-level workspace information architecture.
- Canonical left-rail structure and global navigation views.
- Object model for `team`, `agent`, and communication lanes.
- Right-pane tab model for Team and Agent objects.
- Route grammar, deep-link behavior, and shell-level backward compatibility.
- Narrow-screen and mobile pane behavior for the unified shell.
- Product constraints that preserve current Team and Agent strengths.
- Frontend rollout direction for route and shell convergence.

## Non-Goals

- Replacing Team execution vocabulary or Team runtime semantics.
- Replacing Agent runtime/ACP contracts.
- Defining final API payloads for every new navigation surface.
- Full visual design tokens or detailed component implementation.

## Architecture

### 1) Core Product Principle

AgentHub should not merge `Team` and `Agent` into one domain object.

Instead, AgentHub should expose a unified `Workspace` shell with:

- shared global navigation;
- shared object directory patterns;
- object-specific content panes;
- stable Team-specific and Agent-specific feature depth.

This keeps domain truth intact while removing unnecessary product fragmentation.

### 2) Canonical Workspace Object Model

The unified shell should treat the following as first-class workspace entities:

- `channel`
  - human-facing communication lanes such as `# all`
  - Team review/coordination lanes that are not themselves canonical task objects
  - detailed Team-local channel/thread behavior lives in
    [team-channels-threads.md](./team-channels-threads.md)
- `thread`
  - a focused collaboration context rooted inside a channel
  - not a primary shell entity, but a reusable recent-activity/event unit that can be indexed later
  - must stay subordinate to `channel` in the shell even if a future “recent threads” view is added
- `team`
  - the collaboration boundary with conversation, tasks, runs, and members
- `agent`
  - the execution/runtime boundary with profile, workspace, ACP, and activity

Important constraint:

- `team` and `agent` stay distinct entity types with different contracts
- the unified shell only normalizes how operators discover, open, and switch among them

### 3) Canonical Global Navigation

The unified workspace shell should use one shared set of top-level views:

- `Channels`
- `Tasks`
- `Members`
- `Search`

Guidance:

- these are workspace-level lenses, not Team-only or Agent-only tabs
- they cut across entities instead of living under one page type
- they should become the canonical home for cross-cutting browsing and discovery

### 4) Canonical Left Rail

The left rail should be split into two conceptual layers.

#### 4.1) Global Lens Header

The upper navigation row or compact left-rail header should expose:

- `Channels`
- `Tasks`
- `Members`
- `Search`

This layer changes the operator lens.

#### 4.2) Entity Directory

The main object directory below should be grouped as:

- `Channels`
- `Teams`
- `Agents`

Examples:

- `Channels`
  - `# all`
  - future shared communication lanes
- `Teams`
  - one entry per Team
- `Agents`
  - one entry per standalone Agent or directly inspectable Team member Agent

Important constraints:

- do not make `Humans` and `Machines` permanent first-class sections in the primary rail yet
- if exposed, those belong in the `Members` global lens, not in the default object directory
- the directory should stay compact and content-led, consistent with the current Notion-style Team
  shell

### 5) Canonical Right Pane Behavior

The right pane should always show one opened entity or one opened global lens.

The shell should reuse one frame:

- page header
- compact tab row
- primary content region
- optional secondary side panel or detail dock

But the actual tabs remain entity-specific.

#### 5.1) Team Entity Tabs

A Team should preserve its current strengths. The primary tabs should be:

- `Channels`
- `Kanban`
- `Execution Runs`
- `Members`

Secondary or advanced tabs may include:

- `Overview`
- `Agent ACP`
- `Steps`
- `Mailbox`
- `Debug`

Constraints:

- `Kanban` remains the canonical Team task lane
- `Channels` remains the human-facing Team communication lane
- `Execution Runs` remains an execution/debug lens, not the primary ownership surface

#### 5.2) Agent Entity Tabs

An Agent should become a richer first-class object, not just a card in the Agents page.

The primary tabs should be:

- `Tasks`
- `Workspace`
- `Profile`
- `Activity`

Optional advanced tabs may include:

- `ACP`
- `Execution Runs`
- `Debug`

Constraints:

- `Workspace` should preserve the current AgentHub strength of file/workdir awareness
- `Activity` should preserve runtime/event introspection

### 6) Team-Specific Semantics That Must Be Preserved

The unified shell must not flatten Team semantics into generic communication/task UI.

Specifically:

- humans still interact with Team intent primarily through conversation
- coordinator/runtime still own canonical Team task materialization
- `run` and `step` remain execution/debug artifacts instead of the primary collaboration object
- Team mailbox, actor identity, and run continuity contracts remain unchanged

UI consequence:

- we may learn from Slock-style "message can become task" affordances
- but Team UI should not directly bypass the canonical task-first Team contract
- Team channel/thread rollout should use a focused split view (`channel timeline + thread pane`)
  instead of turning Team communication into a flat message log

### 7) Agent-Specific Semantics That Must Be Preserved

The unified shell must also keep Agent-specific execution depth:

- ACP rendering and history
- local runtime lifecycle
- workspace path and file context
- node/workdir/worktree identity
- standalone execution history

UI consequence:

- Agent pages should become shell-consistent with Team pages
- they must not be reduced to lightweight member-profile cards

### 8) Notion-Style Product Constraints

The unified shell should preserve the recent AgentHub visual direction:

- content-first layouts
- restrained chrome
- thin headers and compact tabs
- low badge density
- low dashboard-card density
- stable typography hierarchy

Explicit anti-goals:

- do not drift toward a Slack-like thick control shell
- do not over-segment the screen with too many persistent panels
- do not make runtime/debug chrome louder than the primary content

### 9) Route Model Direction

The rollout should converge current top-level routes toward one workspace shell.

Target direction:

- one top-level workspace route shell
- one shared left rail and global lens header
- Team and Agent entities opened within that shell

Recommended front-end entity view model:

- `WorkspaceEntity = channel | team | agent`
- `WorkspaceLens = channels | tasks | members | search`

Important constraint:

- `thread` is intentionally not part of `WorkspaceEntity` or the top-level lens grammar in v1
- if a later `Threads` view is added, it should behave as an index of recent active thread contexts,
  not as a new root object type

This is a frontend shell convergence first, not a backend contract rewrite.

### 10) Canonical Route Grammar

The unified shell should converge toward one canonical shell route family.

Preferred direction:

- `/workspace`
  - canonical workspace shell entry
- `/workspace/channels/:channel_id`
  - opens a `channel` entity
- `/workspace/teams/:team_id`
  - opens a `team` entity
- `/workspace/agents/:agent_id`
  - opens an `agent` entity

Lens and tab state should remain additive rather than encoded into many top-level route families.

Preferred query parameters:

- `lens=<channels|tasks|members|search>`
- `tab=<entity-local-tab>`
- `panel=<secondary-panel>`

Compatibility rules:

- `/` should remain a backward-compatible alias to `/workspace` during rollout
- existing `/teams` and `/teams/:team_id` routes may continue as compatibility entry points during
  migration
- existing Agent-root entry points should continue to resolve into the same shell instead of
  redirecting operators into a different product

### 11) Deep-Link Contract

The unified shell should preserve stable deep-link semantics across entity and lens boundaries.

Rules:

- an entity deep link should restore both the entity and its current local tab when possible
- a global lens deep link should not require a Team or Agent entity to be preselected
- cross-object transitions should preserve enough context to return to the previous entity without
  losing the operator's place
- run-scoped Team panels may still require `run_id`, but shell-level routes must not force a run
  when the selected surface is not run-scoped

Examples:

- opening a Team from `Tasks` should allow returning to `Tasks` without resetting the shell lens
- opening an Agent from `Members` should preserve the current workspace shell and use Agent-local
  tabs only inside the right pane
- `Execution Runs` remains a Team-local tab, not a global shell lens

### 12) Narrow-Screen And Mobile Contract

The unified shell must preserve the current Team mobile direction instead of reintroducing a
desktop-only split layout.

Rules:

- narrow screens should behave as two panes, not one long stacked dashboard
- the entity rail and the workspace content pane should be switchable from the header
- the current primary workflow must always stay one tap away:
  - `Channels`
  - `Kanban` for Team
  - Agent primary tabs for Agent objects
- global lenses should remain reachable without requiring a permanently visible wide desktop rail

Constraints:

- do not make desktop left-rail density a hard dependency for navigation
- do not bury primary workflow switches in overflow menus on narrow screens

### 13) Migration Guardrails

The rollout must not regress current semantic contracts.

Guardrails:

- do not rewrite Team task/run/step semantics just to fit the new shell
- do not rewrite Agent ACP/runtime contracts just to fit the new shell
- do not require backend schema rewrites for shell convergence phases
- do not remove existing `/teams` or `/` entry points until the unified shell has stable parity
- do not make Team advanced/debug tabs louder than primary workflow tabs
- do not turn the shell into a Slack-style thick chrome layout

### 14) Rollout Order

The rollout should happen in phases.

#### Phase 1: Shell Convergence

- keep existing Team and Agent inner surfaces mostly intact
- introduce one shared workspace shell
- converge top-level navigation and route selection
- land canonical shell language (`Workspace`) and compatibility aliases
- keep current Team/Agent page internals embedded rather than rewritten

#### Phase 2: Cross-Cutting Lenses

- introduce canonical `Channels`, `Tasks`, `Members`, and `Search` workspace-level views
- wire Team and Agent objects into those shared views
- keep `thread` as a channel-level secondary pane instead of a global shell lens
- keep entity-local tabs distinct from global lenses

#### Phase 3: Agent Object Promotion

- promote Agent from card/list item into a first-class object with `Workspace`, `Profile`, and
  `Activity`
- align Agent object chrome with Team object chrome inside the same shell

#### Phase 4: Rail Convergence

- replace app-level split navigation with one shared left rail
- converge `Channels`, `Teams`, and `Agents` into one compact entity directory
- preserve Team-specific member and workflow affordances as secondary navigation, not as a second
  competing app shell

#### Phase 5: Workflow Refinement

- refine Team/Agent transitions
- add object-to-lens deep links
- tune mobile and narrow-pane behavior

## Validation Matrix

The rollout should be considered successful only when:

- operators no longer need to think in separate `Agents app` vs `Teams app` terms
- Team task-first semantics remain intact
- Agent workspace/runtime depth remains intact
- the shell remains visually compact and Notion-like
- narrow screens still preserve direct access to the primary workflow lanes

Suggested validation surfaces:

- route-shell smoke tests for workspace navigation
- Team and Agent entity open/switch flows
- shared-lens navigation tests (`Channels`, `Tasks`, `Members`, `Search`)
- Chrome DevTools MCP verification for desktop and compact layouts

Suggested minimum validation by phase:

- Phase 1:
  - route-selection tests
  - header/menu tests
  - MCP confirmation that `/workspace` is a valid shell entry
- Phase 2:
  - global-lens smoke tests
  - entity-to-lens and lens-to-entity navigation tests
- Phase 3:
  - Agent object tab tests
  - Agent shell parity checks for `Workspace` / `Profile` / `Activity`
- Phase 4:
  - shared-rail navigation tests
  - compact-layout rail switching tests
- Phase 5:
  - Team/Agent workflow continuity checks
  - mobile and narrow-pane regression checks

## Open Risks

- shell convergence can accidentally blur Team-local tabs and workspace-global lenses
- Agent promotion can accidentally reduce ACP/runtime depth if the shell over-abstracts entity tabs
- a shared left rail can become visually heavier than the current Notion-style Team shell unless
  chrome stays restrained

## Source References

- `docs/features/agents-teams.md`
- `docs/features/frontend-design.md`
- `docs/features/team-execution-vocabulary.md`
