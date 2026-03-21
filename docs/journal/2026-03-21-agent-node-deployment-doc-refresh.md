# Agent Node Deployment Doc Refresh

## Summary

Updated the user-facing deployment and operations docs for remote Agent Nodes so
they match the current internal gRPC control-plane implementation.

## Changes

- expanded `userdocs/docs/deployment/overview-and-topology.md`
  - added distributed-node prerequisites
  - added a concrete `internal_grpc` config example
  - added a rollout order for main control plane + remote nodes
  - extended the post-deploy smoke checklist with a remote-node validation step
- expanded `userdocs/docs/core/agent-nodes.md`
  - added deployment prerequisites
  - added a concise operator rollout flow
- expanded `userdocs/docs/getting-started/configuration-basics.md`
  - documented the optional `internal_grpc` block required for remote Agent
    Nodes

## Notes

- The docs now reflect the current contract that remote-target agent creation
  fails fast when internal gRPC peer configuration is unavailable.
- The node registry remains routing-only; bootstrap/auth material stays in the
  cluster-level internal gRPC configuration.
