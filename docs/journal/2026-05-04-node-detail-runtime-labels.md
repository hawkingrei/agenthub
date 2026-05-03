# Node Detail Runtime Labels

## Summary

Improved the first node detail slice by making attached-agent runtime/provider labels explicit on
the canonical `Agents on this node` roster.

## Background

The node detail route already exposed:

- object header;
- `Info`;
- `Connect Command`;
- attached-agent roster;
- remote-node settings and danger zone.

But the attached-agent rows still stopped at:

- name;
- status;
- worktree summary;
- workdir;

That left one product gap versus the node detail spec: operators could see which agents were
attached to a node, but not what runtime/provider surface each attached agent was actually using.

## Scope

- keep the existing node detail/backend contracts unchanged;
- add explicit runtime/provider labels to attached-agent rows;
- keep the implementation inference-only for now, derived from current agent runtime fields;
- add focused regression coverage around the new labels.

## Key Decisions

- the node detail page now derives attached-agent runtime labels from existing agent fields instead
  of waiting for a new backend payload:
  - `AgentHub Runtime`
  - `Codex CLI`
  - `Gemini CLI`
  - fallback `Custom Runtime`
- labels are intentionally additive:
  - an agent may show both `AgentHub Runtime` and `Codex CLI`
  - unknown commands fall back to `Custom Runtime` instead of rendering nothing
- this remains a UI inference layer only; it does not claim host-probed runtime truth

## Validation

```bash
cd web && npm exec vitest -- run src/components/agent_node_detail_shared.test.ts src/components/agent_nodes_workbench.test.tsx
cd web && npm exec tsc -- --noEmit
```

## Follow-Ups

- if node/runtime discovery grows a canonical backend payload later, replace the current inference
  labels with persisted runtime/provider identity
- consider surfacing the same runtime/provider chips in the compact inline `Agents` node-management
  surface if operators need parity there
