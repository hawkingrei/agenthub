---
sidebar_position: 5
---

# Run Tasks and Interact

## Start a Task

1. Select an agent card
2. Click **Start**
3. Wait for status to become running

If a task is already running, AgentHub keeps a single active runtime for that
agent session to avoid duplicate execution.

## Send Instructions

Use the input dock in the output panel:

- Enter task instructions
- Submit with **Send**
- Use **Interrupt** when an in-progress tool call or long response must stop

## Session Persistence

AgentHub persists runtime state in the backend process:

- Closing or refreshing the browser does not stop the running task
- You can reconnect to the same session later and continue interaction

## Practical Prompting Pattern

- Start with one concrete goal
- Add constraints (path, files, test scope, style rules)
- Ask for verification commands and output summaries
- Iterate in small steps for safer review and rollback
