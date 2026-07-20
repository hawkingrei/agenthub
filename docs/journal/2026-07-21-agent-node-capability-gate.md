# Agent Node Capability Gate

## Summary

Agent node management routes now use the `nodes:manage` capability instead of the root-only
compatibility gate, while bootstrap join information remains root-only.

## Background

The access-control contract allows `root` and `admin` users to manage node records, but not
`operator`, `viewer`, or `device` users. The agent-node routes still used `require_root`, which kept
normal node operations tied to instance-configuration authority.

## Scope

- Switched agent node create, list, get, update, and delete authorization to
  `require_capability(..., NodesManage)`.
- Kept `/api/agent_nodes/bootstrap` on `require_root` because it exposes bootstrap join material.
- Added route coverage proving `operator` is denied, `admin` can list nodes, and `admin` still
  cannot read bootstrap join information.

## Key Decisions

- Treat node registry CRUD as `nodes:manage`, matching the stable capability matrix.
- Keep bootstrap join info as root-only because it is closer to instance/node credential bootstrap
  than routine node operation.
- Reuse the canonical authz helper instead of adding route-local role checks.

## Validation

```bash
cargo test -p agenthub api::agent_nodes::tests::agent_node_routes_require_nodes_manage_capability -- --nocapture
cargo test -p agenthub api::agent_nodes::tests::get_agent_node_bootstrap_requires_root -- --nocapture
cargo test -p agenthub api::agent_nodes::tests -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue migrating normal operator route clusters from root-only gates to capability gates.
- Keep bootstrap token, root lifecycle, safe path, and passkey configuration routes root-only unless
  the stable access-control contract changes.
