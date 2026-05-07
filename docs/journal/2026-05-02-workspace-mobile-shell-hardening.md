# Workspace Mobile Shell Hardening

## Summary

- tightened the shared workspace header so mobile layouts place status/menu actions on their own row
- reduced narrow-screen squeeze in the node detail surface by removing one fixed-width team metric block and by making key metric grids switch earlier to 2-up layouts
- added focused regression coverage for workspace header, nodes detail, compact Team detail routes,
  and browser-level mobile shell flows
- split the compact browser regression path into a dedicated mobile Playwright suite/pipeline instead
  of keeping it buried inside the broader Web E2E batch

## Background

The top backlog item for frontend product work is to make Team, Agents, and Nodes primary flows
hold up on smaller screens instead of relying on desktop layouts that happen to wrap.

Recent shell work already introduced compact Team behavior, but two small risks remained:

- the shared workspace header still packed title, status, and menu chrome too tightly on mobile
- the node detail screen still used a few width assumptions that could over-compress metric and
  member drill-down blocks on narrow screens

## Scope

This slice only hardens the shared shell and the nodes detail page.

It does not yet:

- redesign the full Team mobile information architecture
- change Team or Agent runtime behavior

## Key Decisions

- keep the existing compact Team one-pane behavior intact and use it as the baseline integration
  flow
- harden the shared workspace shell first so Team and Nodes inherit better narrow-screen behavior
  without adding another page-specific layout system
- prefer earlier 2-column metric transitions and `min-w-0` layouts over adding more custom mobile
  CSS
- keep compact browser coverage isolated in its own Playwright file so mobile-only regressions are
  triaged as a separate CI signal

## Validation

- `cd web && npm exec vitest -- run src/components/workspace_shell_header.test.tsx src/components/agent_nodes_workbench.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && PLAYWRIGHT_MOBILE_ONLY=1 npx playwright test tests/e2e/team_page_mobile.e2e.ts --project chromium`
- `PLAYWRIGHT_NO_WEBSERVER=1 PLAYWRIGHT_MOBILE_ONLY=1 npm --prefix web run e2e -- tests/e2e/team_page_mobile.e2e.ts --grep "team setup actions stay reachable"`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`
- Chrome DevTools MCP baseline:
  - inspected deployed Team workspace shell structure at `/workspace/teams/...`
  - confirmed the current header/lens/sidebar composition before edits
  - used that baseline to target the shared header lane split instead of only page-local tweaks

## 2026-05-06 Mobile Setup Coverage

- added a mobile browser regression that creates a shell-only Team, verifies both setup entry
  points remain reachable in the Team setup card, and exercises `Copy Existing Agent`
- locked the disabled `Move to Team (later)` boundary in the mobile dialog so copy and move do not
  collapse under compact controls
- kept this as coverage-only work; Team creation, adoption semantics, and runtime ownership rules
  remain unchanged

## Follow-Ups

- The broader `P0+` mobile-first backlog is now closed by
  [2026-05-07-p0-plus-closure.md](./2026-05-07-p0-plus-closure.md).
- Continue any deeper Agents route chrome refinement as part of the remaining workspace-shell P0
  work, not as a separate P0+ blocker.
