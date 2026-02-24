# Bazel-First Domain Crate Decomposition Policy

## Background

AgentHub already supports Bazel CI (`bazel build //...` and `bazel test //...`), but
Rust implementation still mixes domain code in `src/*` and partially extracted
workspace crates. This makes Bazel target ownership and long-term module
boundaries harder to reason about.

## Scope

- Update project charter constraints in `AGENTS.md`.
- Define a Bazel-oriented Rust decomposition policy.
- Clarify bootstrap entrypoint placement (`src/main.rs` thin, composition in
  `src/app.rs` and domain crates).

## Key Decisions

1. Treat Bazel as a first-class Rust build/test path.
2. Prefer domain-oriented crate extraction under `crates/<domain>`.
3. Keep crate boundaries cohesive; avoid tiny utility crates without clear
   ownership.
4. Align Bazel package boundaries with crate boundaries by default.
5. Keep `src/main.rs` thin and move business logic into library crates.

## Validation

- Documentation-only change.
- Verified updates exist in `AGENTS.md` under:
  - `Technical and Architecture Constraints`
  - `Key Architecture Decisions`
  - `Directory Plan`
  - `Requirement Additions`
  - `TODO`
  - `Change Log`

## Follow-up

- Apply this policy incrementally when touching legacy `src/*` domains.
- Require each migration PR to include both Cargo and Bazel boundary updates.
