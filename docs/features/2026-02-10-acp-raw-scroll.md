# ACP Raw Events Auto-Scroll

## Background

ACP debug raw events should auto-scroll to the latest entry when the Raw tab is active.

## Scope

- Attach the auto-scroll ref to the scrollable raw events list.
- Ensure only one scroll container is active in the raw view.

## Key Decisions

- Keep the wrapper as a layout container and move scrolling to the `<ul>`.

## Validation

Open ACP Debug -> Raw Events and confirm new entries scroll to the bottom.
