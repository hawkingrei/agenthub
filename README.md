# AgentHub

AgentHub is a single-binary service for managing and interacting with remote AI agents.
It provides a Rust backend, an embedded React web UI, ACP-based structured output rendering,
and SQLite-backed persistent sessions.

## What You Can Do

- Create, start, stop, and delete agents
- Run long-lived tasks that continue after the browser tab closes
- Inspect ACP events (messages, plans, tool calls, command output, debug stream)
- Run multi-agent workflows with A2A team orchestration
- Receive completion notifications in the web UI
- Keep operational history with auditable logs and persisted session state

## Documentation Map

- End-user docs site: `userdocs/`
- Internal engineering notes: `docs/features/`
- Internal backlog / follow-ups: `docs/todo.md`
- API payload naming conventions: `docs/api_naming.md`

For user-facing documentation preview/build:

```bash
cd userdocs
npm install
npm run start   # local preview
npm run build   # static output at userdocs/build
```

## Requirements

- Rust (stable)
- Node.js 20+ (for web and userdocs build)
- Bazel / Bazelisk (optional, for Bazel-driven checks)

## Quick Start (Local)

```bash
# 1) Build web assets
cd web
npm install
npm run build

# 2) Run AgentHub
cd ..
cargo run
```

Default UI address: `http://localhost:8080`.

## Configuration

AgentHub reads config from `config.toml`.

```toml
listen_addr = "0.0.0.0:8080"

safe_paths = [
  "/home/foo",
  "/home/foo/projects"
]
```

Runtime state (database, local artifacts) defaults to `~/.agenthub/`.

## Common Development Commands

```bash
# Run Rust tests
cargo test

# Run frontend unit tests
cd web
npm test

# Lint frontend
npm run lint

# Bazel checks (optional)
cd ..
bazel build //...
bazel test //...
```

## Internal Proto Codegen Guard

`proto/internal/v1/team.proto` is compiled by `build.rs` (`tonic-build`) to produce
a fresh reference output in `OUT_DIR` for drift checking.
Application/runtime compilation uses the tracked generated file
`src/internal/proto/agenthub.internal.v1.rs`, which must stay in sync with schema
and generator output.

```bash
# regenerate tracked proto file from latest codegen output
make proto-gen

# verify codegen consistency with tracked generated file
make proto-check
```

## Repository Layout

```text
agenthub/
  src/                # Rust server
  web/                # Vite + React frontend
  userdocs/           # Docusaurus user documentation site
  agenthub-codex-acp/ # ACP adapter workspace member
  docs/               # Internal engineering docs and change notes
  migrations/         # SQLite migrations
```

## License

MIT
