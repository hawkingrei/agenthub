# AgentHub Codex ACP Adapter

AgentHub's built-in Codex ACP adapter. It integrates with the official OpenAI
Codex Rust crates and provides full ACP capabilities, including tool calls,
permissions, commands, plans, modes, and MCP servers.

## Usage

For new AgentHub agents, use the daemon's built-in provider worker:

```
agenthubd acp codex
```

Stored bare compatibility commands are rewritten to the daemon worker at spawn
time:

```
agenthub-codex-acp
```

## Notes

- This package is library-only and is embedded in `agenthubd`.

## License

Apache-2.0

See [NOTICE](NOTICE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for
adapter provenance and preserved third-party notices.
