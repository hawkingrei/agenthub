---
title: Agents Panel Scroll
date: 2026-02-10
status: implemented
---

## Summary

Confine agents list scrolling to the left panel instead of the entire page.

## Background

When the agents list grows, the entire workspace can start scrolling, which
makes the output panel jump and hides the input dock. The goal is to keep
scrolling within the agents list only.

## Decision

- Make the agents panel a flex column container.
- Constrain the agents list to the available height and enable internal scroll.

## Scope

- `web/src/styles.css`

## Validation

- [ ] Add enough agents to overflow the panel and confirm the list scrolls
  inside the left panel.
- [ ] Confirm the workspace itself no longer scrolls when the list grows.
