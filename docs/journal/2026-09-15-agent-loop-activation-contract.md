# Agent Loop Activation Contract

## Summary

Define the [activation implementation contract](../features/agent-loop-activation-contract.md)
before introducing durable storage or provider execution. This is the first of the implementation
slices following the product-definition checkpoint.

## Background

The existing watchdog needs a live ACP handle. Startup cleanup cancels active Team runs and can
reopen linked tasks. Durable activation therefore requires explicit ownership, compatibility, and
cleanup rules instead of reusing process status or reminder receipt meanings.

## Scope

- Identity and mailbox partition lifetime, additive storage boundaries, and finite policy defaults.
- Separate admission suspension, structured outcome, task acceptance, and process cleanup.
- A deterministic SQLite/fake-process experiment for the critical transaction boundaries.
- Implementation sequencing in the active TODO; production lifecycle migration remains pending.

## Key Decisions

- New loops require explicit enablement, initially local Team members with fresh-session defaults.
- Preserve mailbox partitions and task-attempt semantics across temporary provider sessions.
- Keep an expired executor reserved until its writing authority is verifiably removed.
- Commit outcome and continuation before finish acknowledgment; release only after cleanup.
- Record redacted lifecycle evidence from the first storage slice. Production history remains
  separate from debug-only doctor diagnostics.

## Validation

Focused design experiment: `python3 scripts/check_loop_activation_contract.py`.
It covers transaction rollback, a wake racing with finalization and DB reopen, expired ownership,
a second SQLite connection, and stale finish/cleanup generations. This experiment does not prove
production runtime correctness; subsequent Rust integration tests must reproduce those boundaries.

Documentation checks: `git diff --check` and relative-link/required-section inspection.

## Follow-Ups

Implement the remaining storage, admission, lifecycle, tool, prompt, UI, app, and Rara slices in
[TODO](../todo.md#agent-loop-product-transition), with focused evidence in each behavior PR.
