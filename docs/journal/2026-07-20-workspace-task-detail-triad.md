# Workspace Task Detail Triad

- Date: 2026-07-20

## Summary

Extended the shared workspace split-pane primitive into the desktop Team task board so selected task
context can sit beside the Kanban work zone instead of living only in a modal.

## Background

The unified workspace shell contract already defines a desktop three-zone model: object directory,
primary work zone, and right-side context zone. Team channel threads used the shared split-pane
primitive, but the task board still kept selected task detail behind a modal-only path.

## Scope

- Keep the existing compact/mobile modal task-detail path intact.
- Add a desktop task-detail dock through the shared `WorkspaceSplitPaneLayout` primitive.
- Add a split-pane variant marker so tests can distinguish thread context from task detail context
  without relying only on local Team class names.
- Update the workspace IA contract to name task detail preview as part of the shared split-pane
  contract.

## Key Decisions

- The task board remains the primary work zone.
- The selected task detail dock is desktop-only chrome; compact workspaces still use the modal
  detail interaction.
- The shared split-pane primitive keeps the existing `1.45fr / 0.9fr` desktop ratio for both
  channel thread context and task detail context.

## Validation

- `cd web && npm run test -- src/components/layout/workspace_section_shell.test.tsx src/pages/team_panels.test.tsx`

The isolated worktree did not have `web/node_modules`, so validation first ran `cd web && npm ci`
with `npm_config_cache=/private/tmp/agenthub-npm-cache`.

## Follow-Ups

- Collect deployed browser evidence for Team navigation surfaces after the current production
  entrypoint is healthy.
- Continue moving remaining Team context docks, such as member profile or inspector surfaces, onto
  shared workspace primitives when they need persistent desktop context.
