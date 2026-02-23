# Linkerdog ACP

Standalone ACP runtime entrypoint for Linkerdog.

## Usage

Run ACP runtime directly:

```bash
linkerdog-acp
```

Optional compatibility form (no behavioral difference):

```bash
linkerdog-acp acp
```

## Runtime Overrides

Set default provider/model/mode via `-c key=value`:

```bash
linkerdog-acp -c provider=openai -c model=gpt-5 -c mode=code
```

Supported keys:

- `provider`, `linkerdog.provider`, `agent.provider`
- `model`, `linkerdog.model`, `agent.model`
- `mode`, `linkerdog.mode`, `agent.mode`

## Architecture

- Shared runtime/session engine lives in `crates/linkerdog-core`.
- This package is only ACP argument parsing and ACP runtime bootstrapping.
- `linkerdog-cli` can also route to this runtime via `linkerdog acp`.
