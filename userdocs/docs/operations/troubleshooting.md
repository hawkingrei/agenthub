---
sidebar_position: 8
---

# Troubleshooting

## Cannot Login

- Verify username/password pair
- Check whether bootstrap/join is completed
- Confirm backend is reachable and auth config is valid

## Agent Cannot Start

- Confirm workdir is under allowed `safe_paths`
- Check whether the agent is already running
- Review server logs for process spawn errors

## No Output or Stale Output

- Re-open the session and check status in agent cards
- Switch to Debug/Raw output tabs for transport-level events
- Verify server process is still alive

## Worktree Problems

- Ensure selected repository path exists and is accessible
- Check branch/ref availability when using create-worktree flows
- Clean up abandoned worktrees if disk usage grows unexpectedly

## Recovery Strategy

1. Keep the session for audit and replay
2. Create a fresh run with a clean workdir/worktree
3. Replay the prompt sequence in smaller steps
