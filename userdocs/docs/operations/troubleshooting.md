---
sidebar_position: 4
---

# Troubleshooting

## Cannot Login

- Verify username/password pair
- Check whether bootstrap/join is completed
- Confirm backend is reachable and auth config is valid

Quick checks:

```bash
curl -i http://localhost:8080/
```

## Agent Cannot Start

- Confirm workdir is under allowed `safe_paths`
- Check whether the agent is already running
- Review server logs for process spawn errors

Typical causes:

- Invalid workdir path
- Missing executable/provider command
- Process permission constraints

## No Output or Stale Output

- Re-open the session and check status in agent cards
- Switch to Debug/Raw output tabs for transport-level events
- Verify server process is still alive

Recovery action:

1. Keep the current failed session for evidence
2. Create a fresh session with the same prompt
3. Compare behavior and isolate environment differences

## Worktree Problems

- Ensure selected repository path exists and is accessible
- Check branch/ref availability when using create-worktree flows
- Clean up abandoned worktrees if disk usage grows unexpectedly

## Recovery Strategy

1. Keep the session for audit and replay
2. Create a fresh run with a clean workdir/worktree
3. Replay the prompt sequence in smaller steps
