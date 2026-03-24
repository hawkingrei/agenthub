# Documentation Surface Compaction

## Summary

Compacted the documentation surface so `docs/todo.md` returns to an active
backlog, older documentation-only micro-journals are merged into one background
record, and the user-facing docs now expose clearer product, feature, and
architecture entry points.

## Background

Documentation had drifted in three ways:

1. `docs/todo.md` had become a mixed ledger of active work, completed checks,
   and stale verification bullets.
2. Several early `userdocs` and README journals still described setup history
   rather than serving as useful standalone references.
3. The published docs site had solid task-level pages, but it lacked a strong
   overview layer that explains what AgentHub is, which surfaces matter, and
   how the system fits together.

## Scope

- `docs/todo.md`
- `docs/README.md`
- `README.md`
- `userdocs/README.md`
- `userdocs/docusaurus.config.js`
- `userdocs/sidebars.js`
- `userdocs/docs/intro.md`
- `userdocs/docs/overview/*`
- `userdocs/docs/getting-started/installation.md`
- `userdocs/docs/getting-started/configuration-basics.md`
- `userdocs/docs/advanced/team-workbench.md`

## Key Decisions

1. Treat `docs/todo.md` as an active backlog only.
   Completed items are removed after evidence lands elsewhere; duplicated
   rollout checks are collapsed into umbrella items.
2. Keep stable product and engineering truth in layered surfaces:
   - `userdocs/` for user-facing workflows and architecture introductions
   - `docs/features/` for durable engineering contracts
   - `docs/journal/` for dated implementation checkpoints
3. Merge documentation-only bootstrap and refresh journals into this background
   record once they stop carrying distinct technical decisions.
4. Strengthen the user docs landing layer with explicit overview pages:
   product overview, feature overview, and architecture overview.
5. Align published docs metadata with the real site URL
   `https://doc.agenthub.hawkingrei.com/`.

## Superseded Journals

The following journals were merged into this background note because their
useful conclusions are now reflected directly in `userdocs/`, `README.md`, or
`docs/README.md`:

- `docs/journal/2026-02-15-docusaurus-userdocs-site.md`
- `docs/journal/2026-02-15-userdocs-build-root-link-fix.md`
- `docs/journal/2026-02-15-userdocs-ci-build-check.md`
- `docs/journal/2026-02-15-userdocs-content-expansion.md`
- `docs/journal/2026-02-16-readme-docs-structure-refresh.md`
- `docs/journal/2026-02-16-userdocs-deployment-advanced-expansion.md`
- `docs/journal/2026-03-22-readme-refresh.md`

## Resulting Documentation Model

- `README.md` is the repository entry point and should explain the project in
  one screen, then route readers to the right docs surface.
- `userdocs/` is the published user documentation site and must cover:
  - product overview
  - feature overview
  - architecture overview
  - setup, usage, deployment, and operations
- `docs/features/` is the canonical engineering contract layer.
- `docs/journal/` keeps dated checkpoints and compaction background.
- `docs/todo.md` tracks only open follow-up work.

## Validation

Recommended checks for this documentation refresh:

```bash
npm --prefix userdocs run build
```

Expected outcomes:

- Docusaurus builds without broken links
- the sidebar exposes the new overview layer
- README and docs index link to the published docs site and current local build
  workflow

## Follow-Ups

- Add screenshots or diagrams for the new overview pages once the main Team and
  Agent Node surfaces stabilize further.
- Keep future documentation-only journal compactions explicit so link history is
  still auditable.
