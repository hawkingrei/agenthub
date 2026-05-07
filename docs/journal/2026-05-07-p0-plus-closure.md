# P0+ Closure

## Summary

- closed the remaining P0+ backlog items after the channel/thread closure PR merged
- confirmed the small-screen Team/Workspace contract now has Team, Agents, Nodes, and dedicated
  mobile browser CI coverage
- confirmed Team existing-agent adoption has landed as copy-first, with move and deeper copy modes
  explicitly deferred outside the P0+ closure

## Background

The active objective is to finish P0+ work before moving on to P0. After the metadata projection and
channel/thread closure PRs landed, two P0+ items remained open in `docs/todo.md`:

- small-screen and mobile-first Team/Workspace design
- Team adoption for existing agents without collapsing `copy` and `move`

Both had implementation and validation evidence spread across feature specs, journals, web tests,
and CI workflow definitions. This note records the closure audit so the backlog can move those
items out of active P0+ without implying that lower-priority follow-ups are already done.

## Scope

This closure is documentation-only.

It does not:

- add new mobile UI behavior
- enable `move existing agent to Team`
- add workspace-content copy or memory/context seeding for copied Team agents
- close the remaining P0 workspace-shell, typecheck, deployed verification, or performance items

## Key Decisions

- Treat small-screen support as closed for P0+ because the contract now has:
  - shared workspace header narrow-screen lane splitting
  - Team compact single-column regression coverage
  - Agents mobile primary-control coverage
  - Nodes mobile detail coverage
  - a dedicated `Web E2E Mobile` GitHub Actions workflow on `push` and `pull_request`
- Treat Team adoption as closed for P0+ because the first rollout contract is copy-first:
  - source agents remain unchanged
  - copied Team members get new Team-owned agent identities
  - empty-Team copy creates the coordinator
  - later copy creates workers
  - `Move to Team` is not a default executable action yet
- Keep adoption extensions as P1 follow-up work rather than letting the P0+ item remain open:
  - stopped-only `move existing agent to Team`
  - explicit workspace-content copy
  - explicit memory/context seeding

## Validation

Evidence inspected for small-screen closure:

```bash
sed -n '1,280p' web/tests/e2e/team_page_mobile.e2e.ts
sed -n '1,80p' .github/workflows/e2e-mobile.yml
sed -n '1,120p' docs/journal/2026-05-02-workspace-mobile-shell-hardening.md
```

The mobile E2E file covers:

- Team single-column proportions and workspace header lane split on a 390px viewport
- Nodes detail stacking without horizontal overflow
- Agents workbench input/send controls and panel switching on mobile
- Team setup copy flow and worker add path on mobile

The workflow file confirms the dedicated mobile browser CI path:

```bash
npm run e2e:coverage:mobile
```

Evidence inspected for Team adoption closure:

```bash
sed -n '1,280p' docs/features/team-agent-adoption.md
sed -n '1,110p' docs/journal/2026-05-06-team-adoption-move-deferred.md
sed -n '130,210p' web/tests/e2e/team_page_setup.e2e.ts
rg -n "copyExisting|Copy Existing|copy.*agent|deleteAgent|updateTeamSpec|coordinator|worker|source" \
  web/src/pages/team/use_team_management_actions.test.tsx \
  web/src/pages/team/team_management_modals.test.tsx \
  web/src/pages/team/forge_helpers.test.ts \
  web/src/pages/team_setup_panel.test.tsx \
  web/src/pages/team_panels.test.tsx
```

The adoption evidence covers:

- modal copy semantics and source-agent unchanged copy
- first copied member as coordinator
- later copied member as worker
- missing source and conflict cleanup behavior in management actions
- browser-level Team setup adoption flow
- explicit deferral of `Move to Team`

Relevant PR CI evidence:

```bash
gh pr checks 530 | cat
```

PR #530 showed the dedicated `Web E2E Mobile` check passing before merge, alongside the normal web,
Rust, P2P, and Bazel gates.

## Follow-Ups

- Continue with the remaining P0 backlog after this closure PR merges.
- Keep Team adoption move/workspace-copy work as a separate P1 project with runtime and ownership
  guardrails before implementation.
