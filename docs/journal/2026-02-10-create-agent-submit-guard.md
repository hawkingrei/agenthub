---
title: Create Agent Submit Guard
date: 2026-02-10
status: implemented
---

## Summary

Prevent duplicate agent creation when the Create Agent button is clicked multiple
times before the request completes.

## Background

The Create Agent modal previously allowed repeated clicks while the create
request was in flight, which could spawn duplicate agents and leave the modal
open long enough for double submission.

## Decision

- Add a create-in-flight guard in `web/src/app.tsx` to reject re-entry.
- Pass the busy state into the modal to disable and show loading on the Create
  button (and disable Cancel while busy).

## Scope

- `web/src/app.tsx`
- `web/src/components/create_agent_modal.tsx`
- `web/src/create_agent_modal.test.tsx`

## Validation

- [ ] Click Create Agent repeatedly while the request is pending and confirm
  only one agent is created.
- [ ] Confirm the Create Agent button shows loading and is disabled while the
  request is in flight.
- [ ] Confirm the modal closes after a successful create.
