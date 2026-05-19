---
name: agenthub-testing
description: "Trigger when a behavior change, bugfix, or CI failure needs the smallest useful AgentHub test plan."
---

# AgentHub Testing

Trigger when changing behavior in AgentHub and deciding what tests to add, update, or run.

## Non-Negotiable Rules

### 1. Every bugfix needs a test

Follow the repo rule:

- every bugfix must include a corresponding test
- the test should reproduce the fixed behavior or lock the regression boundary

Do not merge a behavior fix with only "manual verification" if a focused automated test is practical.

### 2. Prefer the narrowest meaningful test surface

Pick the smallest validation set that actually covers the changed behavior.

Examples:

- ACP loading/cache bug:
  - focused `vitest` on `acp_history_prefetch`, `use_team_actions`, `team_member_acp_panel`
- Team page callback stability:
  - focused `vitest` on the affected hook/panel/smoke test
- Rust domain behavior:
  - focused `cargo test -p <crate> <test_name>`
- DB migration/index path:
  - focused crate test plus `cargo fmt --all --check`

Do not default to the whole suite when one or two files can prove the change.

### 3. Web changes should usually clear all three gates

For non-trivial `web/` changes, the default local validation target is:

```bash
cd web && npm run lint
cd web && npm exec tsc -- --noEmit
cd web && npm run build
```

Then add focused `vitest` files for the changed behavior.

### 4. CI failures must be classified

When a PR goes red, distinguish:

- test bug
- implementation bug
- workflow/config bug
- unrelated flaky infrastructure

Do not paper over an implementation regression by weakening the test unless the test is genuinely wrong.

## Test Selection Guide

### Web ACP / Team UI

Common files:

- `web/src/pages/team/use_team_actions.test.tsx`
- `web/src/pages/team/use_team_member_acp_effects.test.tsx`
- `web/src/pages/team_member_acp_panel.test.tsx`
- `web/src/acp_panel.test.tsx`
- `web/src/hooks/use_acp_conversation.interaction.test.tsx`
- `web/src/pages/team_page.smoke.test.tsx`

Use these when the change affects:

- loading/empty/rendered states
- history prefetch
- cache handoff
- top-edge loading
- tab state
- mailbox/member ACP routing

### Rust backend

Prefer:

- focused `cargo test -p <crate> <name>`
- `cargo check -p <crate>`
- `cargo fmt --all --check`

If the change touches DB schema or migrations, add or update a migration regression test.

### Bazel-facing changes

If the change affects Rust crate boundaries or shared build targets, ensure the code remains Bazel-friendly, but keep local validation narrow unless the bug is specifically Bazel-related.

## Test Design Heuristics

### 1. Reproduce the real shape

Use the real failing shape when possible:

- exact payload fields
- session ids
- mixed event types
- duplicate refresh timing
- cache warm/cold states

### 2. Prefer table-driven decision tests

If a bug comes from many booleans or state combinations, extract a pure helper and test the decision matrix directly.

### 3. Lock the regression boundary, not just the happy path

For each fix, try to cover:

- the failing case
- the closest non-failing neighbor

That usually means one positive and one negative assertion.

## Typical Validation Sets

### Web behavior fix

```bash
cd web && pnpm exec vitest run <focused test files...>
cd web && npm run lint
cd web && npm exec tsc -- --noEmit
cd web && npm run build
```

### Rust behavior fix

```bash
cargo test -p <crate> <focused test name> -- --nocapture
cargo fmt --all --check
```

### CI follow-up

When a PR already exists, after pushing:

- check review threads
- check GitHub checks
- fix the smallest true blocker first

## Review And Coverage Discipline

- If review feedback identifies a real missing regression boundary, add the test in the same PR.
- If patch coverage is just below threshold, prefer adding narrow tests around the changed files rather than broad unrelated suites.
- If a fix introduced a reusable rule, consider whether it belongs in a pure helper plus table-driven tests.
