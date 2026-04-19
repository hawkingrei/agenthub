# Team Channels And Threads Specification

## Problem

The current Team workspace exposes one shared conversation lane (`# all`) plus task/run/member
surfaces, but it does not yet support a compact `channel + thread` interaction model.

This leaves several gaps:

- Team communication still feels like one flat conversation instead of a set of discoverable
  communication lanes;
- replies are not elevated into a first-class thread context with a stable right-side pane;
- task affordances are distributed across the page instead of staying close to the message composer;
- the unified Workspace shell already borrows Slock-like object navigation, but Team communication
  has not yet learned the corresponding `channel / thread` information architecture.

We want to learn from the Slock channel/thread layout without collapsing AgentHub Team semantics or
losing the current Notion-style content-first shell.

## Scope

- Team communication-lane information architecture.
- Canonical `channel` and `thread` shell behavior inside Team workspaces.
- Team actor capability direction for opening and replying in threads.
- URL/deep-link behavior for channel and thread selection.
- Right-side thread pane behavior.
- Composer-level task affordance direction.

## Non-Goals

- Replacing Team canonical task ownership or Team run/step semantics.
- Defining the full backend storage schema for every future channel feature.
- Implementing reactions, rich moderation, or arbitrary Slack-style channel administration.
- Turning human chat input directly into canonical Team task records.

## Architecture

### 1) Core Principle

Team should support a `channel timeline + thread pane` structure.

The shell should follow this shape:

- left rail: Team directory plus Team channels;
- center pane: selected channel timeline;
- right pane: optional selected thread;
- composer: always attached to the currently selected conversation context.

This should feel structurally closer to Slock while still preserving AgentHub-specific Team depth.

### 2) Canonical Entity Model

Inside a Team workspace, communication should distinguish:

- `channel`
  - a human-facing communication lane inside one Team
  - examples:
    - `# all`
    - future review / topic / onboarding channels
- `thread`
  - a focused reply context rooted in one channel message
  - always belongs to one parent `channel`
- `task`
  - the canonical Team work object
  - remains distinct from both `channel` and `thread`

Important constraint:

- channels and threads are communication/review surfaces
- they are not the canonical Team task surface

### 3) Channel Contract

The Team shell should treat channels as first-class Team communication lanes.

Required baseline:

- `# all` remains the default channel
- `# all` is the broad coordination/default lane, not the only lane
- left rail groups channels under `Channels`
- selecting a channel replaces the center timeline without leaving the Team workspace shell
- channel header stays light and directory-like:
  - channel name
  - optional short description
  - optional compact member/visibility metadata

Multi-channel contract:

- a Team may expose multiple channels in addition to `# all`
- every non-default channel should have a short, explicit work-scoping description
- channels should narrow context, not duplicate Kanban/task ownership
- non-default channels may be created explicitly by the Team leader or a human operator
- non-default channels may also be archived or deleted explicitly when the work lane is no longer
  needed

Recommended examples:

- `# all`
  - default coordination lane
  - human requests, planning, announcements, cross-cutting updates
- `# review`
  - focused review / approval / PR / decision follow-up
- `# research`
  - investigation notes, paper or issue exploration, structured findings
- `# rollout`
  - launch, migration, deployment, operational checklists

Constraints:

- do not create channels as arbitrary folders for every task
- channels should describe a durable work mode or work stream, not replace canonical Team tasks
- each channel description should make it obvious why a message belongs there instead of `# all`
- `# all` is the system default channel and must not be deletable
- deleting a non-default channel must not delete canonical Team tasks or execution history
- deleting a non-default channel should remove only the communication lane and its thread context,
  or archive it first if hard deletion is too destructive for the current rollout phase

Constraints:

- do not add thick dashboard chrome or channel-level runtime metrics that Slock does not need
- keep channel rows compact: `title + one compact meta line` at most

### 4) Thread Contract

Threads should become first-class secondary conversation contexts.

Canonical meaning:

- `channel`
  - the durable Team communication lane
- `thread`
  - a focused collaboration context rooted in one event inside a channel
  - initially, that root event is a channel message
  - later, the same concept may also index review/execution follow-up events without changing the
    channel-first shell

Product consequence:

- thread is not the primary Team entrypoint
- thread is the reusable “recent active context” unit that can be reopened from either:
  - the current channel timeline
  - a future recent-threads index
- this matches the useful part of the Slock model without collapsing AgentHub into a chat-only
  product

Required behavior:

- selecting a reply target opens a right-side `ThreadPane`
- the center channel timeline remains visible
- the right pane header should show:
  - `Thread`
  - parent channel reference (for example `#papers`)
  - `View in channel`
  - `Close thread`
- thread messages render as a focused sub-conversation instead of expanding inline into a large
  nested tree

Important constraint:

- a thread is a communication context, not a separate Team object type
- a thread is not a top-level Workspace lens and must stay subordinate to its parent channel
- closing the thread should return the layout to channel-only mode without losing the selected
  channel
- the first version should derive `thread` from an existing channel root message instead of
  inventing a detached empty thread object
- a later `Threads` or `Recent threads` view, if introduced, should be an index of active thread
  contexts rather than a new creation surface

### 5) Actor Capability Contract

`Open thread` should not remain only a front-end affordance.

It should become a Team actor capability with shell projection.

Recommended capability surface:

- `team_channel_create`
  - input:
    - `team_id`
    - `channel_id`
    - `title`
    - optional `description`
  - behavior:
    - creates a non-default Team communication lane
    - reserved for Team leader and human operator flows by default
- `team_channel_update`
  - input:
    - `team_id`
    - `channel_id`
    - optional `title`
    - optional `description`
    - optional ordering / visibility metadata
- `team_channel_archive`
  - input:
    - `team_id`
    - `channel_id`
  - behavior:
    - hides a non-default channel from the active rail without destroying canonical Team work
- `team_channel_delete`
  - input:
    - `team_id`
    - `channel_id`
  - behavior:
    - permanently removes a non-default communication lane when explicit deletion is desired
    - must reject deletion of `# all`

- `team_thread_open`
  - input:
    - `team_id`
    - `channel_id`
    - `root_message_id`
    - optional `reason`
  - behavior:
    - if a thread already exists for the root message, return it
    - otherwise lazily materialize/open a thread bound to that root message
- `team_thread_reply`
  - input:
    - `team_id`
    - `channel_id`
    - `thread_id` or `root_message_id`
    - reply content
- `team_thread_view_in_channel`
  - shell/navigation projection back to the parent channel timeline

Important constraints:

- `team_channel_create` / `team_channel_delete` should default to leader + human operator authority
- worker actors should not create or delete Team channels unless a later policy explicitly grants it
- thread open must be anchored to an existing channel message
- agent actor must not create detached empty threads without a `root_message_id`
- thread capability does not change canonical Team task ownership or task materialization rules
- the shell `Open thread` button should become a projection of this actor capability rather than an
  isolated UI-only construct

### 6) URL And Deep-Link Contract

The Team workspace should support stable deep links for channels and threads.

Target direction:

- `/workspace/teams/:team_id?channel=:channel_id`
- `/workspace/teams/:team_id?channel=:channel_id&thread=:thread_id`

Guidance:

- `channel` controls the center timeline
- `thread` controls the optional right-side pane
- a missing `thread` means the thread pane is closed
- a missing `channel` defaults to the Team default lane (`# all`)
- `thread` should initially resolve from the root channel message identity
  (`root_message_id`-backed deep link)

### 7) Composer Task Affordance

We should learn from Slock's composer-adjacent `As Task` affordance, but keep AgentHub semantics.

Required constraint:

- human chat input must not directly create canonical Team task records

Recommended direction:

- composer may expose a lightweight task affordance such as:
  - `Create task draft`
  - `Promote to task request`
- this affordance should live next to the message composer, not as a thick page-level toolbar
- resulting behavior should still flow through Team planning/runtime materialization before becoming
  a canonical Team `task`
- non-default channels may shape the draft/request context, but must not bypass leader/runtime
  canonicalization

### 8) Relationship To Existing Team Semantics

This design must preserve the existing Team operating model.

Still true after rollout:

- human intent enters through conversation
- leader/runtime own canonical task materialization
- `Kanban` remains the canonical Team task lane
- `Execution Runs` remains the canonical execution-history/debug lane
- `run` and `step` remain execution/debug artifacts, not communication objects

### 9) Notion-Style Constraints

The channel/thread rollout must keep the current visual direction:

- restrained chrome
- thin headers
- compact directory rows
- low badge density
- content-first panes

Explicit anti-goals:

- do not add Slack-like thick composer toolbars
- do not add persistent runtime/status panels around every channel
- do not overload thread headers with debugging metadata

## Rollout Phases

### Phase 1: Channel/Thread Spec And Routing

- define canonical Team `channel` and `thread` route/query grammar
- introduce shell-level selected-channel / selected-thread state
- keep the current single channel (`# all`) behavior working through the new shell model

### Phase 2: Thread Split View

- add a real right-side `ThreadPane` in Team conversation
- keep the center channel timeline visible
- wire message reply/thread-count affordances to open/close the pane

### Phase 3: Actor Capability Integration

- add Team actor-level `thread_open` / `thread_reply` behavior
- make shell thread open/close a projection of actor-side thread identity instead of pure UI state

### Phase 4: Additional Team Channels

- make `Channels` in the Team rail a true Team-local directory instead of a fixed two-item list
- support more than `# all` while preserving current Team defaults
- require channel descriptions that explain the work focus of each non-default lane

### Phase 5: Composer Task Draft Affordance

- add a lightweight task affordance near the composer
- keep canonical task creation behind Team planning/runtime semantics

## Validation

- route-level tests for `channel` and `thread` query/deep-link behavior
- Team shell tests for `channel-only` vs `channel + thread` layouts
- conversation tests for opening and closing the right thread pane
- actor command / capability tests for thread-open anchored to `root_message_id`
- channel-directory tests for `# all` plus descriptive non-default channels
- regression tests confirming `Kanban` and `Execution Runs` stay canonical for task/run ownership
- Chrome DevTools MCP checks against:
  - Slock channel/thread reference page
  - local Team workspace regression after implementation
