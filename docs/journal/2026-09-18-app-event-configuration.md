# App Event Configuration

## Summary

Optional event declarations, safe source metadata, versioned inbound signing-key references, and
explicit member event routes establish the configuration boundary for signed event intake.

## Background

Existing tool grants must not silently become activation permissions. Incoming signatures also
must not reuse outbound MCP credentials or allow ordinary App owners to select daemon secrets.

## Scope

Domain validation, additive registry tables, credential isolation, and authorized configuration
routes. Production event delivery and standing conditions remain unfinished.

## Key Decisions

- Store inbound key references separately and retain prior references for child-environment isolation.
- Permit installation only with instance configuration authority; App owners may inspect safe state
  and revoke. Team owners separately approve classes for current members.
- Pin event routes to both authorization epochs and the selected manifest version. Restore and
  version changes require explicit reapproval.
- Keep notifications bounded to identifiers; reject free-form payloads and execution instructions.
- Key-installation exhaustion cannot block revocation; repeated revocation of the current tombstone
  returns its existing version, keeping retained state bounded.

## Validation

- Root App regressions: 16 pass; the one ignored child fixture executes through its parent.
- Static API authorization guards: 3 pass.
- Complete domain and DB library suites: 15 and 178 pass, including legacy migration/reopen.
- The real controller/provider/CLI shim workflow confirms current, rotated, unbound, and revoked
  signing-key environment variables are absent from both provider and shim processes.
- A standalone HMAC proof using the existing dependency versions matches an independent Python
  vector and rejects tampered inputs, expired timestamps, and truncated signatures.
- All five event configuration storage regressions pass after adding the exhausted-key revocation case.
- Root/domain/DB all-target Clippy passes again with warnings denied. Formatting, whitespace, and
  19 local documentation links pass.

```bash
cargo +1.96.0 test -p agenthub --lib ::apps::
cargo +1.96.0 test -p agenthub --lib api::authz::
cargo +1.96.0 test -p agenthub-agent-domain -p agenthub-db --lib
cargo +1.96.0 test -p agenthub-db --lib app_registry::tests::event_tests
cargo +1.96.0 clippy -p agenthub -p agenthub-agent-domain -p agenthub-db --all-targets -- -D warnings
cargo +1.96.0 fmt --all --check
```

## Follow-Ups

- The [signed intake checkpoint](2026-09-18-app-event-intake.md) supplies authenticated ingress,
  atomic deduplication/cursor/intake, bounded audit, and budgets.
- Integrate standing conditions and safe context/history/doctor attribution, then publish the
  complete event slice and validate current-head CI.

The stable boundary is [App event configuration](../features/app-event-configuration.md).
