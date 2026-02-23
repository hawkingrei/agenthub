# Linkerdog CLI

General Linkerdog binary entrypoint.

## Usage

Start ACP runtime via CLI entrypoint:

```bash
linkerdog
```

or

```bash
linkerdog acp
```

Both forms route to `linkerdog-acp` runtime bootstrapping logic.

## Runtime Overrides

```bash
linkerdog -c provider=anthropic -c model=claude-sonnet-4 -c mode=review
```
