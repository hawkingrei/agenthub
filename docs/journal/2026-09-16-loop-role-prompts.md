# Loop Role Prompts

## Summary

Select one configured coordinator or worker prompt per activation and attach the shared
`team-loop-runtime` skill. Recover work through canonical task/IM tools and finish through the
structured lifecycle contract, without requiring six phases or a resident polling process.

## Background

The common activation entry previously carried recovery pointers but did not select the member's
configured role prompt. The dependency integration checkpoint established scheduling, MCP, and
scoped knowledge before this role-specific composition.

## Scope

- Compact loop templates in `agenthub-team-prompts`; manual templates remain unchanged.
- Member `prompt`/`prompt_append` resolution before immutable launch recording.
- Shared managed loop procedure, real private skill files, and ACP prefix/metadata delivery.
- Provider/CLI regressions for delegation, evidence, acceptance, waits, and configuration changes.

## Key Decisions

- Classification: role boundary, skill pointer, runtime recovery, and structured output contract.
- Entry version `loop-entry-v6` records the role and configured/built-in source. Effective prompt
  and skill bytes participate in the configuration digest; prompt bodies do not enter trace data.
- New role templates remain below 2,000 bytes each. Repeated procedures live in the skill; the
  combined configured role text retains the existing 20,000-byte ceiling.
- Pin the managed skill in an activation-specific private directory. Exclude random paths from
  the digest, retain files across launch clones, and release them with the provider launch.
- Do not attach the legacy managed Team skill graph to loop mode. Workspace extensions remain
  procedural and cannot grant task creation, assignment, acceptance, or lifecycle authority.

## Validation

Focused validation commands:

```bash
cargo +1.96.0 test -p agenthub-team-prompts -p agenthub-managed-skills --locked --offline
cargo +1.96.0 test -p agenthub-acp loop_ --locked --offline
cargo +1.96.0 clippy -p agenthub --all-targets --locked --offline -- -D warnings
cargo +1.96.0 test -p agenthub --lib agent::manager::loop_launch --locked --offline
cargo fmt --all --check
git diff --check
```

The provider fixture uses real signed actor CLI calls and persisted task/activation records.
It covers coordinator delegation, a worker's denied self-acceptance followed by durable evidence,
and coordinator acceptance in a fresh session after reading that evidence. Configuration changes
during provider startup must not alter the selected entry; a later activation sees the new text.
A waiting activation exits with a durable continuation, then recovers to no actionable work in a
fresh session. ACP guards cover multiple provider rounds under one prompt and skill file lifetime.

The final focused selection passes 13 root loop-launch tests, with three isolated child helpers
invoked by their parent tests; 10 ACP loop tests; all seven prompt tests; and all six managed-skill
tests. Root all-target Clippy passes with warnings denied. The actual CLI binary was rebuilt before
the final preflight-only addition; the final root tests execute the updated daemon implementation
and the unchanged CLI protocol. Formatting, whitespace, skill frontmatter, provider script syntax,
and 13 local documentation links pass. Current-head CI remains the delivery gate.
No external model quality or private knowledge service is inferred from deterministic fixtures.

## Follow-Ups

- Finish current-head CI for the role integration.
- Continue the authorized history/diagnostics, UI, App, and upstream-gated Rara slices.
- Canonical contract: [Team system prompts](../features/team-system-prompt-contract.md).
- Dependency evidence: [loop integration](2026-09-16-loop-dependency-integration.md).
