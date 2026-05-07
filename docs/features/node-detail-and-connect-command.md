# Node Detail And Connect Command

## Problem

AgentHub already has a backend node abstraction (`target_node_id`, node registry, remote execution),
but the product surface still treats nodes mostly as configuration attached to agent creation and
admin/node-management forms.

That is too weak for the workflow we actually need:

- operators need to inspect one node as a first-class object;
- disconnected nodes need an obvious recovery path;
- root operators need a stable `connect command` surface instead of hunting through docs or config;
- node detail should explain which agents are attached to the node and what runtimes that node can
  actually host;
- future unified workspace IA needs `node` to be a stable detail page, not just hidden setup state.

The Slock machine detail reference shows the right product direction:

- object-first header;
- compact machine metadata;
- explicit `CONNECT COMMAND`;
- attached agents as a primary section;
- destructive actions moved into a separate low-emphasis section.

## Scope

- define the product model for `node` as a first-class workspace object;
- define the first node detail page information architecture;
- define how `connect command` should appear and when it should be emphasized;
- define the relationship between `node`, `agent`, and `team member` on the UI surface;
- align the current `Agents` node-management surface with the future node detail page.

## Non-Goals

- redesigning Team runtime semantics;
- changing mailbox or actor routing contracts;
- replacing the existing node registry backend schema in one step;
- full visual implementation details for every breakpoint.

## Product Model

AgentHub should treat these as separate objects:

- `node`
  - infrastructure/execution host object;
  - owns connectivity, runtime availability, and host-level defaults;
- `agent`
  - runtime object scheduled onto one node;
  - owns ACP/workspace/execution history;
- `team member`
  - collaboration identity inside a Team spec;
  - may be attached to an agent, and that agent may be attached to a node.

Important constraint:

- `team member` must not absorb `node` identity;
- `node` must not remain only an agent-create form field;
- `agent` is the attachment bridge between collaboration identity and infrastructure placement.

## Node Detail Information Architecture

The first node detail page should use this section order:

### 1) Header

- node display name;
- connectivity state (`online`, `offline`, `degraded`);
- stable node identifier;
- optional last-seen metadata if available.

The header should stay object-first and low-action.
Do not place many control buttons beside the title.

### 2) Name

- editable node name or rename action;
- canonical value shown plainly, not hidden in developer-only metadata.

### 3) Info

This section should answer “what is this node and what can it run?”

Recommended fields:

- OS / arch;
- daemon version;
- update availability if known;
- detected runtimes;
- default worktree root;
- created timestamp;
- last seen timestamp when available.

Observed agent runtimes should render as compact capability tags. The first rollout derives these
tags from attached-agent commands and known operator-facing runtime surfaces; it must not present
missing attached-agent evidence as a host binary installation failure.

- `Codex CLI`
- `Gemini CLI (no attached agent observed)`

This is more useful than hiding runtime support behind raw config text.

For the first rollout slice, it is acceptable to derive these tags from attached-agent runtime
signals plus known operator-facing runtime surfaces, as long as the UI does not overclaim that the
values came from a direct host probe.

### 4) Connect Command

This section is required.

It should include:

- one copyable command;
- a short explanation that the process must remain running;
- state-aware emphasis:
  - when the node is offline, this section should be visually prominent;
  - when the node is online, it should still remain available but lower emphasis.

The section should not be buried behind an admin modal.
It belongs on the node detail page because it is the direct recovery path for the object currently
being inspected.

### 5) Agents On This Node

This section is primary content, not a sidebar note.

Each row should show:

- agent name;
- runtime/provider label;
- current status;
- click-through into agent detail.

Primary actions for this section may include:

- `Create Agent`
- `Start All` / other bulk actions only when they are operationally justified.

### 6) Workspaces / Attachments

Follow-up section for:

- discovered workspaces;
- scan/refresh action;
- future session/workspace attachment summaries.

This section should hold its own actions (`Scan`, `Refresh`) instead of overloading the page header.

### 7) Danger Zone

Low-emphasis destructive section for:

- delete node;
- destructive constraints such as “all attached agents must be removed first”.

## Connect Command Contract

AgentHub should expose a token-first or credential-derived node connect command as product UI, not
just documentation.

The page-level command block should support:

- copy button;
- stable monospace rendering;
- clear ownership of what values are already substituted;
- a note about long-running process expectations.

The connect command should be derived from canonical node/bootstrap data, not handcrafted in the UI
from unrelated text fragments.

## Relationship To Existing Agent Nodes Surface

The current `Agents` page node-management panel is a valid bootstrap surface, but it should evolve
toward one of two roles:

- compact roster + creation surface;
- jump-off surface into canonical node detail pages.

The detailed node inspection experience should move out of one large inline admin panel and into a
dedicated detail view.

That means:

- keep inline node registration/editing where useful for quick setup;
- promote selected-node inspection into a stable node detail page;
- reuse the same section model there (`Info`, `Connect Command`, `Agents On This Node`, `Danger`).

## Relationship To Unified Workspace IA

This spec intentionally does not make `node` a default primary rail entity yet.

However, it establishes `node` as a first-class inspectable object so future workspace IA can choose
one of these paths without rethinking the object model:

- expose nodes under `Members` or `Operations`;
- expose nodes in an admin/infrastructure sub-surface;
- later promote nodes into the shared object directory if product scope expands.

## First Rollout Slice

The first shippable slice should be:

1. define a dedicated node detail route/view-model;
2. surface `Info` and `Connect Command`;
3. render attached agents as a primary list;
4. keep destructive actions in a separate section;
5. link from the current `Agents` node-management surface into this detail page.

Do not block this slice on:

- multi-node topology redesign;
- Team runtime attachment rewrite;
- remote ACP parity improvements.

## Validation

Manual validation should cover:

1. offline node detail shows a prominent `Connect Command` section;
2. copy action returns the full command exactly;
3. online node still exposes the command without dominating the page;
4. attached agents list matches the selected node;
5. create-agent flow can preserve the currently selected node context;
6. delete-node affordance stays separated from normal operational actions.

Chrome DevTools MCP checks should record:

- node detail URL;
- visible section order;
- whether `Connect Command` is visible above the fold when offline;
- whether attached agents read as primary content instead of admin metadata.

## Related Specs

- [agent-nodes.md](./agent-nodes.md)
- [workspace-unified-ia.md](./workspace-unified-ia.md)
