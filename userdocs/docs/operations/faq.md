---
sidebar_position: 5
---

# FAQ

## Why does the task continue after I close the browser?

AgentHub runs sessions in backend-managed processes. Browser disconnect does not
terminate the runtime by design.

## Should I use one agent per repository?

Not always. Use one agent per stable intent or workflow. For parallel work,
separate agents and worktrees are easier to reason about.

## What is the safest default for daily usage?

Use `create_worktree` mode with predictable naming and clean up stale runs
regularly.

## Why do I get path permission errors?

Most often the path is outside `safe_paths` or resolved path differs from what
you expect. Confirm configured safe path list first.

## How do I debug missing output events?

1. Check agent state in cards
2. Inspect Debug/Raw stream
3. Verify server process and logs
4. Retry from a fresh session if stream state is unrecoverable

## How do I keep long sessions readable?

- Keep prompts narrow
- Split large tasks into phases
- Start a new session when context drifts too far
