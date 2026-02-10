---
title: Mantine UI Library POC
date: 2026-02-09
status: implemented
---

## Summary

Introduce Mantine as the first UI library experiment for AgentHub, adding a
shared theme and migrating key modals to Mantine components to standardize
inputs, buttons, and layout primitives.

## Background

The web UI relies on bespoke HTML and CSS. Adding new flows requires repeating
form styles and modal layouts. A component library should reduce duplication and
make UI development more consistent.

## Decision

- Add `@mantine/core` and `@mantine/hooks` to the web dependencies.
- Wrap the app in `MantineProvider` with a theme aligned to existing typography
  and radius tokens.
- Migrate the Create Agent and Permission Requests modals to Mantine components.
- Keep existing layout CSS while scoping base element styles to avoid overriding
  Mantine styling.

## Scope

- `web/package.json`
- `web/src/main.tsx`
- `web/src/ui/mantine_theme.ts`
- `web/src/components/create_agent_modal.tsx`
- `web/src/components/permission_modal.tsx`
- `web/src/styles.css`
- `web/src/permission_modal.test.tsx`

## Validation

- Run `npm install` in `web/` to refresh dependencies.
- Run `npm run dev` and confirm the Create Agent and Permission modals render
  with Mantine styling.
- Verify permission options with empty `option_id` remain disabled.
