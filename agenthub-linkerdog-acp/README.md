# AgentHub Linkerdog ACP Agent

Standalone `linkerdog` ACP agent package for AgentHub.

This package now runs a native Linkerdog runtime (not a thin delegation to
`agenthub-codex-acp`).

## Usage

Run ACP directly:

```bash
linkerdog
```

Or with explicit subcommand:

```bash
linkerdog acp
```

Both entry forms start the same runtime.

## Runtime Overrides

You can set default provider/model/mode via `-c key=value`:

```bash
linkerdog -c provider=openai -c model=gpt-5 -c mode=code
```

Supported keys:

- `provider`, `linkerdog.provider`, `agent.provider`
- `model`, `linkerdog.model`, `agent.model`
- `mode`, `linkerdog.mode`, `agent.mode`

## Current Capabilities

- Multi-provider and model selectors via ACP session state/config options.
- Session mode switching (`ask` / `code` / `review`).
- Append-only session context persisted at:
  - `<cwd>/.cache/context/run/<session_id>/state.json`
  - `<cwd>/.cache/context/run/<session_id>/history.jsonl`
- Basic tool-call flow with permission request and local command execution:
  - user prompt: `/tool exec <command>`

## Notes

- This crate is independently runnable and independently buildable.
- Current model response path is local runtime logic; remote LLM provider adapters
  can be layered on this runtime in follow-up iterations.
