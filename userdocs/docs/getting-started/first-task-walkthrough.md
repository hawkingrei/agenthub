---
sidebar_position: 4
---

# First Task Walkthrough

This walkthrough verifies the installed server, ACP adapter, workspace safety,
live output, and history replay in one short run.

## 1. Sign In

Open `http://localhost:8080` by default and sign in. A new instance first asks
you to initialize the root operator.

## 2. Create an Agent

1. Open **Agents** and select **Create Agent**.
2. Set a clear name, such as `docs-pilot`.
3. Choose the `agenthub-acp codex` preset or another installed ACP provider.
4. Choose a workspace mode:
   - `create_worktree` for isolated repository work.
   - `use_existing` when you intentionally want to use an existing directory.
   - `reuse_worktree` when an existing Git worktree is already prepared.
5. Confirm the selected path is under a configured `safe_paths` root.
6. Create the agent and select its card.

## 3. Start and Send an Instruction

Select **Start**, wait for the agent to become `running`, and send a bounded
instruction from the input dock:

```text
Summarize the current README and propose three concrete improvements. Do not edit files.
```

Use **Interrupt** if the active turn needs to stop. Interrupting a turn is not
the same as deleting the agent or its history.

## 4. Review the Result

- Use **Thread** for the user-facing conversation.
- Use **Plan** when the provider emits structured plan updates.
- Enable developer mode from **Admin** only when you need the **Debug** tab or
  session metadata.

The exact tool, mode, model, and configuration controls depend on the selected
ACP provider.

## 5. Verify Reconnect

Refresh the browser, reopen the agent, and confirm that its status and previous
output return. Browser disconnects do not terminate the backend-managed
process; AgentHub replays persisted events and resumes live SSE updates.

## Done Criteria

- The server accepts login.
- Both `agenthub` and the selected ACP provider are available.
- The workspace passes safe-path validation.
- The task produces structured output.
- History remains visible after refresh.
