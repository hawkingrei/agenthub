---
title: Agents Row Compact Layout
date: 2026-02-10
status: implemented
---

## Summary

Reduce the agents row visual density to match the previous compact layout.

## Background

After recent UI updates, the agents list felt too tall and consumed excessive
vertical space. The goal is to restore a tighter row height without removing
important metadata.

## Decision

- Reduce padding and typography sizes in the agents list rows.
- Tighten vertical gaps in the row header and metadata section.
- Keep the header and workdir on a single line with ellipsis to avoid height
  expansion when names or paths are long.
- Prevent grid rows from stretching by aligning the agents list content to the
  start.

## Scope

- `web/src/styles.css`

## Validation

- [ ] Confirm agents rows match the previous compact height and fit more entries
  before scrolling.
