---
sidebar_position: 4
---

# First Task Walkthrough

This page walks through one full run from login to result review.

## Goal

Create one agent, run one concrete coding task, and confirm output is persisted.

## Step 1: Login

1. Open AgentHub in your browser (`http://localhost:8080` by default).
2. Sign in with username and password.
3. Confirm you can see the Agents page.

## Step 2: Create an Agent

1. In the top form, set a clear name, for example `docs-pilot`.
2. Pick command/preset for your provider.
3. Select workdir mode:
   - Use `create_worktree` for isolated work.
   - Use `use_existing` for an existing local repo path.
4. Submit and confirm a new agent card appears.

## Step 3: Start and Send Instruction

1. Click **Start** on the new agent.
2. Wait until status changes to running.
3. In input dock, send one bounded instruction, for example:

```text
Summarize the current README and propose 3 concrete improvements.
```

## Step 4: Review Output

1. Use **Conversation** to read final answer.
2. Use **Debug / Raw** only if behavior looks wrong.
3. If needed, press **Interrupt** to stop a long in-progress operation.

## Step 5: Reconnect Check

1. Refresh the browser tab.
2. Re-open the same agent/session.
3. Confirm previous conversation and status are still available.

This verifies backend-side session persistence.

## Done Criteria

- Agent created successfully
- Task reached a terminal state (`completed` or `failed`)
- Output history remains visible after reconnect
