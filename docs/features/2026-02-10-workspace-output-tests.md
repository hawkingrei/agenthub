# Workspace Output Tests

## Background

Output header, ACP debug, and agents panel layout changes increased UI complexity. Existing coverage only exercised a few ACP runtime paths.

## Scope

- Add React render tests for `OutputHeader`, `AcpPanel`, `AcpDebug`, and `AgentsPanel`.
- Update CSS guard assertions for output body spacing.

## Key Decisions

- Use `renderToStaticMarkup` to avoid introducing new testing dependencies.
- Keep coverage focused on layout-critical text and section toggles.

## Validation

```bash
cargo test -p agenthub web_assets
cd web && npm test
```
