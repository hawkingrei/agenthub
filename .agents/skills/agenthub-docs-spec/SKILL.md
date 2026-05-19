---
name: agenthub-docs-spec
description: "Trigger when updating `docs/features/*.md` for stable runtime, UI, or API contracts instead of rollout history."
---

# AgentHub Feature Spec Writing

Trigger when writing or reviewing `docs/features/*.md` files in this repository.

## Goal

Keep feature specs as the stable source of truth for:

- product and engineering scope
- runtime or UI contracts
- architecture boundaries
- validation expectations
- known operational risks

Feature specs are not date-stamped rollout notes and should not read like a changelog.

## Surface Rules

- Canonical specs live under `docs/features/`
- Filenames should be topic-oriented kebab-case
- No date prefixes in `docs/features/`
- Chronological implementation notes belong in `docs/journal/`
- Open verification or rollout tails belong in `docs/todo.md`

## Required Structure

Each active feature spec should include:

- `Problem`
- `Scope`
- `Non-Goals`
- `Architecture`
- `Contracts`
- `Validation Matrix`
- `Operational Notes`
- `Open Risks`
- `Source Journals`

If a section is intentionally short, keep it short, but do not silently drop it.

## Writing Rules

### 1. Capture stable contracts, not transient implementation trivia

A spec should answer:

- what behavior is intended
- what boundary is canonical
- what consumers may rely on

Avoid low-signal content such as:

- PR-by-PR change logs
- temporary debugging notes
- CI run chatter
- file-by-file implementation inventory without contract value

### 2. Keep scope and non-goals explicit

Do not let a spec silently expand into adjacent product surfaces.

Examples:

- if a feature is currently single-operator only, say so
- if multi-user or QR onboarding is out of scope, say so
- if a runtime tag is derived rather than backend-detected, say so

### 3. Contracts must describe the canonical path

When there is a transition period, distinguish:

- canonical path
- compatibility path

Example shape:

- canonical route: `/workspace/nodes/:node_id`
- legacy compatibility: `?lens=nodes&node=...`

Do not leave both paths sounding equally primary unless that is truly intended.

### 4. Validation should be product-aware

The `Validation Matrix` should describe the focused checks needed to trust the spec:

- focused Rust tests
- focused web tests
- typecheck/lint/build gates
- browser/MCP verification when relevant
- push/PR CI evidence when rollout closure depends on it

### 5. Source journals should point back to implementation evidence

Every non-trivial spec should link the journals that established or evolved it.

This lets the spec stay compact while journals retain the chronology.

## Compaction Guidance

When multiple journals describe the same area:

1. Move durable conclusions into the feature spec
2. Leave rollout details and validation evidence in journals
3. Point `docs/todo.md` follow-ups at the canonical spec, not at drifting scratch context

## Decision Boundary

Create or update a feature spec when:

- product behavior changed
- UI/UX contracts changed
- runtime/API boundaries changed
- a backlog item needs a stable canonical target before implementation

Do not use a spec for:

- one-off implementation diary notes
- transient cleanup progress
- unresolved brainstorming with no accepted contract

## Before Finishing

Check these:

- filename is topic-oriented, not date-oriented
- scope and non-goals are explicit
- contracts describe the post-change canonical behavior
- validation matrix is concrete
- source journals are linked
- any remaining rollout tail lives in `docs/todo.md`, not only in prose
