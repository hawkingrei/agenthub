# Dependabot Security Remediation

## Summary

This checkpoint removes all 16 dependencies reported by the repository's open Dependabot alerts
from the resolved AgentHub dependency graphs. It covers the Rust ACP runtime, the web application,
and the user documentation site without dismissing or accepting any advisory.

The Rust remediation is pinned to the exact Codex security patch commit
`6ca61345ceb09d76edc3db8c4eb55df18a10888a` and SACP compatibility commit
`c731bb045d1375af48b0446af728aea52503b30b`. The user documentation site replaces the archived
`image-size` implementation with a repository-owned compatibility package backed by
`probe-image-size` because no patched upstream `image-size` release exists.

## Background

The open alerts covered these dependency groups:

- Rust: `gix`, `gix-fs`, `gix-pack`, `hickory-proto`, `jsonwebtoken`, `opentelemetry_sdk`, `rmcp`,
  and `tar` through the default Codex ACP runtime;
- user documentation: `image-size` and `uuid`; and
- web application: `@babel/core`.

Upgrading AgentHub's previous Codex 0.146 pin to the upstream 0.147 tag removed its old `rmcp`
release but still resolved affected Git, DNS, JWT, telemetry, and archive dependencies. The Claude
ACP adapter also resolved `rmcp` 0.12 through SACP 10.1. The default ACP adapter reaches both
graphs, so transitive ownership was not sufficient evidence to leave the alerts open.

## Scope

- Pin all Codex Cargo and Bazel inputs to the same security-patched fork commit.
- Adapt the AgentHub Codex ACP bridge to the small protocol changes introduced by Codex 0.147.
- Patch SACP 10.1 to use `rmcp` 1.8 while retaining the API required by `claude-code-acp-rs`.
- Patch the Codex dependency graph to resolve:
  - `gix` 0.83.0, `gix-fs` 0.21.1, and `gix-pack` 0.70.0;
  - `hickory-proto` and `hickory-resolver` 0.26.1;
  - `jsonwebtoken` 10.4.0 with the RustCrypto backend;
  - `opentelemetry_sdk` 0.32.1;
  - `rmcp` 1.8.0 and 3.0.0; and
  - `tar` 0.4.46.
- Replace the userdocs `image-size` package with a local compatibility package backed by
  `probe-image-size` 7.3.0 and pin `uuid` 11.1.1.
- Refresh the web and userdocs npm lockfiles, including `@babel/core` 7.29.7 and the current
  `brace-expansion` fix discovered by `npm audit`.
- Refresh the Cargo lockfile and validate Bazel crate-universe generation against the same graph.

## Key Decisions

- Keep the existing `image-size` import surface used by Docusaurus. The compatibility package is
  deliberately narrow: it supports the synchronous buffer/path API and the asynchronous
  `fromFile` API that the current build consumes.
- Preserve the Rama 0.3 alpha API used by Codex instead of migrating the entire Rama stack. The
  fork vendors only `rama-dns` and adapts it to `hickory-resolver` 0.26.1, which keeps the security
  change reviewable.
- Use a renamed Reqwest 0.13 dependency only for OTLP transport. Codex continues to use Reqwest
  0.12 elsewhere, while `opentelemetry-http` 0.32 receives the client version it implements.
- Forward Cargo package aliases into Bazel Rust targets so renamed external crates remain
  unambiguous when two versions are present.
- Remove AgentHub's V8 crate-universe annotation because Codex 0.147 no longer resolves the V8
  crate. Retaining the annotation makes lock generation fail as an unused compatibility override.
- Use immutable fork commits rather than branch references. Return to upstream Codex and SACP
  releases after they contain equivalent resolved graphs and pass the AgentHub ACP compatibility
  suite.

## Validation

- `cargo check -p agenthub-codex-acp-runtime`
  - passed against Codex 0.147 at the pinned patch commit.
- `cargo test -p agenthub-codex-acp-runtime`
  - passed: 133 tests.
- SACP compatibility fork validation:
  - `cargo check -p sacp` passed;
  - `cargo test -p sacp` passed: 37 integration/unit tests and 66 doc tests, with 1 integration
    test and 23 doc tests ignored by the upstream suite.
- Codex fork Cargo checks and focused tests for Git, DNS/network proxy, OTLP, JWT identity, plugin
  archives, and RMCP passed.
- Codex fork Bazel validation:
  - OTLP unit and integration targets passed after external dependency aliases were wired;
  - Git utilities, network proxy, agent identity, and OTLP targets passed in the combined run;
  - the upstream `core-plugins` suite passed under Cargo but one Bazel-only transport-error test
    remained sensitive to local socket-close behavior (`384 passed, 1 failed`). The failure does
    not exercise the dependency upgrades or AgentHub code.
- AgentHub Bazel validation:
  - `bazel mod deps --lockfile_mode=update` completed after reusing the locked Cargo dependency
    cache and confirmed the V8 annotation was obsolete;
  - the Codex and unified ACP adapter test targets progressed through dependency analysis, but the
    local run did not reach compilation because crate-universe repeatedly fetched the same Codex
    fork commit for each Git package and GitHub transfers stalled. CI remains the terminal Bazel
    gate for these two targets.
- `npm run test:image-size-compat` in `userdocs/`
  - passed: in-memory PNG, file PNG, and malformed zero-length HEIF/ICNS container cases.
- `npm run build` and `npm audit --json` in `userdocs/`
  - passed; audit reported zero vulnerabilities.
- `npm run lint`, `npm run build`, and `npm audit --json` in `web/`
  - passed; audit reported zero vulnerabilities.
- Resolved dependency inspection confirmed none of the alerting Rust package versions remain in
  the AgentHub ACP runtime graph.

## Follow-Ups

- Confirm GitHub closes all 16 alerts after this change reaches the default branch and Dependabot
  rescans the new lockfiles.
- Replace the temporary `hawkingrei/codex` and `hawkingrei/symposium-acp` pins with upstream stable
  releases that provide the same safe dependency graph and pass Cargo plus Bazel ACP validation.
