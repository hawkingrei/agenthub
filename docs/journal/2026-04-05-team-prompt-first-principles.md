# Team Prompt First-Principles Requirement

## Summary

- strengthened both default Team leader and worker prompts with an explicit first-principles reasoning requirement
- made the requirement part of the prompt crate regression contract so later prompt edits do not silently remove it

## What Changed

- `crates/agenthub-team-prompts/prompts/default_team_leader_prompt.txt`
  - added a leader role-policy rule requiring first-principles thinking before planning or delegation
  - added a planning-gate rule that separates fundamentals from implementation accidents
- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
  - added a worker workspace-policy rule requiring first-principles thinking before choosing an implementation path
  - inserted an explicit workflow step that re-derives goals, constraints, invariants, and failure mode before coding
- `crates/agenthub-team-prompts/src/lib.rs`
  - extended prompt contract tests to assert the new first-principles wording for both roles

## Why

- Team prompts already described workflow phases and execution flow, but they did not explicitly require first-principles reasoning.
- That leaves too much room for cargo-culting existing code paths or reacting to symptoms without re-deriving the real problem.
- Encoding the principle in both role prompts keeps planning and execution aligned on the same reasoning standard.

## Validation

- `cargo test -p agenthub-team-prompts`
