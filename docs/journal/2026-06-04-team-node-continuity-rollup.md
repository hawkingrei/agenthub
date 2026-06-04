# Team And Node Continuity Rollup

## Summary

- Refreshed Agent Node deployment docs around token-first join, config-driven node mode, and current `internal_grpc` bootstrap/auth boundaries.
- Finalized the Team workspace memory v1 contract with explicit `L0` / `L1` / `L2` retention, redaction, promotion, and pre-compaction flush ordering.
- Slimmed coordinator and worker default prompt tails by removing duplicated channel/thread guidance while keeping the detailed mailbox, task, and thread contracts intact.
- Closed the documentation/design TODOs that are now represented by stable feature specs, user docs, prompt tests, and this journal.

## Background

The remaining P1 backlog mixed two related closeout tracks:

- Agent Node docs needed to match the current token-first join flow and node startup boundaries.
- Team runtime docs needed one reviewed memory continuity contract before more long-horizon memory implementation work continued.

The prompt tails also still carried duplicate channel/thread wording even though the later runtime contract already contains the precise routing rules and skill pointers.

## Scope

- `docs/features/agent-nodes.md`
- `docs/features/team-workspace-memory-contract.md`
- `userdocs/docs/deployment/overview-and-topology.md`
- `userdocs/docs/core/agent-nodes.md`
- `userdocs/docs/getting-started/configuration-basics.md`
- `userdocs/docs/getting-started/installation.md`
- `userdocs/docs/overview/feature-overview.md`
- `crates/agenthub-team-prompts/prompts/default_team_coordinator_prompt.txt`
- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
- `crates/agenthub-team-prompts/src/lib.rs`
- `docs/todo.md`

## Key Decisions

- Agent Node join is token-first. Root operators copy bootstrap details from `Agents -> Join node with token`; QR/device onboarding is not part of the Agent Node path.
- Bootstrap auth and steady-state internal gRPC auth remain separate. `internal_grpc.bootstrap.token` authenticates join/bootstrap, while `internal_grpc.auth.shared_secret`, `issuer`, and `audience` authenticate internal gRPC control and actor CLI traffic after bootstrap.
- The node registry remains routing-only. It records route metadata such as `grpc_target`, `tls_server_name`, and `default_worktree_root`, not bootstrap tokens, shared secrets, or TLS file paths.
- Team memory v1 is tiered: `L0` prompt working state is ephemeral, `L1` run artifacts stay high fidelity under `.cache/context/run/<run_id>/...`, and `L2` durable memory requires deliberate redacted promotion.
- Pre-compaction flush order is stable: write `L1` evidence, update prompt-visible pointers, promote redacted `L2` summaries, then keep the next prompt tail pointer-first.
- Coordinator and worker prompts should keep only one detailed channel/thread contract. Repeated early summaries were removed; the later direct-mailbox, channel, thread, and reporting-surface guidance remains canonical.

## Validation

Validated in this change:

```bash
cargo test -p agenthub-team-prompts -- --nocapture
cargo fmt --check
npm --prefix userdocs run build
git diff --check
```

`npm --prefix userdocs run build` completed successfully and generated static files. Docusaurus also
printed a local update-check config-store permission warning, which does not affect the build output.

## Follow-Ups

- Real multi-node Team direct-mailbox routing still needs deployed verification.
- Remote-node transport posture remains open for staging validation, same-port HTTP/gRPC multiplexing design, and long-term production identity/mTLS rollout.
