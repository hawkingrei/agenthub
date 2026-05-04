# Node Detail Name Section

## Summary

Split `Name` out of the larger remote-node `Settings` form so node detail reads more like an
object page and less like one merged admin panel.

## Background

The node detail route already had a dedicated object surface, but the editable node name still
lived inside the broader routing/settings form alongside:

- `gRPC target`
- `TLS server name`
- `Default worktree root`

That made the canonical object field harder to scan and kept the page closer to one large settings
editor than the intended detail-page hierarchy from the node detail spec.

## Scope

- add a dedicated `Name` section to node detail;
- keep remote-node rename as a separate save path from route/worktree settings;
- keep the existing backend update contract unchanged;
- leave the local control-plane node name read only from this surface.

## Key Decisions

- remote node detail now exposes:
  - `Name`
  - `Settings`
  - `Danger Zone`
- `Save Name` only persists the canonical display name and reuses the currently persisted route /
  worktree fields for the update payload
- `Save Settings` only persists routing/worktree fields and reuses the currently persisted node
  name for the update payload
- local `Main Node` now shows a dedicated read-only `Name` section instead of skipping the field
  entirely

## Validation

```bash
cd web && npm exec vitest -- run src/components/agent_nodes_workbench.test.tsx
cd web && npm exec tsc -- --noEmit
```
