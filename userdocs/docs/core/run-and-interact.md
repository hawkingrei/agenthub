---
sidebar_position: 3
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

## Session Controls

When the active ACP runtime exposes session controls, the debug surface can also
let you:

- switch `mode` or `model` from provider-supplied options
- submit generic config values
- cancel an actively running ACP turn
- force a new session when recovery is easier than continuing the current one

These controls are runtime-specific. Codex, Gemini, Claude, and other ACP
providers may surface different option sets.

## Claude ACP Runtimes

AgentHub can launch external Claude ACP adapters as normal agents:

| Adapter | Command | Notes |
|---------|---------|-------|
| Claude Agent SDK ACP | `claude-agent-acp` | Provided by `@agentclientprotocol/claude-agent-acp`; runs in ACP mode by default. |
| Claude Code ACP Rust | `claude-code-acp-rs --acp` | Provided by `claude-code-acp-rs`; `--acp` is required for interactive ACP mode. |

Configure Anthropic credentials through the adapter-supported environment or
Claude settings files before starting the agent.

## Pre-Run Checklist

Before sending the first instruction:

1. Confirm target files and repository path
2. State constraints explicitly (style, tests, no API breaks)
3. Define expected output format (patch summary, command list, etc.)

## Session Persistence

AgentHub persists runtime state in the backend process:

- Closing or refreshing the browser does not stop the running task
- You can reconnect to the same session later and continue interaction
- Technical metadata stays available through compact `Details` surfaces instead
  of taking over the main conversation header

## Practical Prompting Pattern

- Start with one concrete goal
- Add constraints (path, files, test scope, style rules)
- Ask for verification commands and output summaries
- Iterate in small steps for safer review and rollback

## Multi-Step Execution Pattern

For larger tasks, use phased prompts:

1. Ask for plan and risk list
2. Ask for implementation of one phase only
3. Ask for validation commands and remaining risks
4. Repeat for next phase
