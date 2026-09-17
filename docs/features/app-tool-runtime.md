# Registered App Tool Runtime

## Problem

Registered tools need the same durable effect journal as configured Mem while retaining their own
manifest, scope, version, and revocation boundary. Provider metadata cannot grant that authority.

## Scope

HTTP MCP Apps bound to local Team loop members, using the
[management API](app-management-api.md), [registry](app-registry-storage.md), and
[shared operation journal](mcp-operation-journal.md).

## Non-Goals

Local App process hosting, remote-member credential delivery, automatic version tracking, a
marketplace, signed event ingestion, and a frontend App management form.

## Architecture

The launch resolver freezes the complete App selection, including an empty selection, for an
activation. Retries reuse those pins. Every App contributes an opaque binding fingerprint and a
local stdio shim descriptor to the existing launch snapshot. Only after recording that snapshot
does the daemon mount the resolved binding in the shared proxy.

The App adapter builds an explicit tool-name access policy from the manifest and granted scopes.
It uses the existing HTTP transport, protocol machinery, budgets, task receipts, and journal.
Resources, prompts, logging, and provider callbacks receive no App authority.

## Contracts

### Version and schema

- Manifest publication and version-only binding changes affect later activations. Existing pins
  keep their original version; permission changes and revocations invalidate those pins.
- Discovery must match the pinned input and output schemas before tools are advertised or admitted.
- Arguments are validated locally and forwarded unchanged. Manifest replay policies are trusted;
  upstream replay hints cannot expand them.
- Completed native results, including errors, are bounded to 1 MiB, 64 levels, and 32,768 nodes.
  Successful `structuredContent` must satisfy the optional output schema. Native error results
  remain exempt from the success schema. Deferred receipts are validated when their result arrives.
- An invalid completed result is an uncertain effect, not proof that a write failed. The shared
  journal retains replay protection and later factual reconciliation.

### Authority and lifecycle

- Open and every subsequent session admission check the durable activation pin and current App,
  Team grant, and member binding epochs. Caller-controlled server references cannot choose an
  endpoint, credential, manifest, or grant.
- Listeners and subscriptions recheck authority before delivery and at one-second idle intervals.
  Revocation closes their delivery path without waiting for another upstream event.
- A write admitted before revocation may finish and retain its factual result. Revocation prevents
  later admissions; it does not rewrite completed effects.
- Scoped close and activation cleanup remain available after App revocation.
- A launch accepts at most 32 local proxy descriptors. The hub retains its global 128-session and
  per-executor 32-session limits, shared byte budgets, and bounded exchange workspaces.

### Credentials and identity

- Connections are provisioned by an instance administrator. HTTPS or loopback HTTP is required.
  Optional credentials resolve daemon-side from validated environment references; invalid or
  missing values produce fixed errors without echoing their contents.
- Loop provider and shim environments exclude credentials for every registered App, including
  unbound and revoked Apps. Descriptors contain a local executable, server ID, and actor credential
  file reference, never an upstream URL, token, or credential environment name.
- Sensitive daemon headers assert `x-agenthub-app-id`, `x-agenthub-app-version`,
  `x-agenthub-team-id`, `x-agenthub-actor-id`, `x-agenthub-activation-id`, and
  `x-agenthub-workspace`. Workspace is a SHA-256 digest of the canonical local workspace path.
  Native argument headers remain under `mcp-param-*` and cannot replace these assertions.
- Stable effect identity uses administrator-provisioned authority and namespace. Version,
  permission revision, and credential changes cannot disguise an uncertain earlier write.
  Configuration fingerprints include manifest/version, scopes, permission epochs, and workspace;
  activation IDs, generation, credential values, and administrative revision increments are excluded.
- Offline preflight validates configuration, secret availability, and shim availability without
  connecting to App endpoints. Startup retries inspect retained selections rather than new bindings.
  Runtime upstream failures remain bounded tool errors; independent actor work can still finish.

### Safe discovery and history

- An authorized Agent Card exposes `bound_apps` with App ID, display name, explicit version, and
  scope-filtered tool names. It describes current configuration, including while a process is absent.
  Only Team owners or active Teamspace members can inspect that Team's App capabilities. Publishing
  alone does not change the list; explicit rebinding does. Revoked grants/bindings/Apps disappear.
- Historical tool summaries derive optional `app: { app_id, version }` attribution from immutable
  activation pins and the existing journal server reference. They retain original versions after
  publication, rebinding, revocation, or process exit. Control RPCs do not acquire App attribution.
- The debug doctor prints the same attribution. Release history APIs remain independent of the
  debug-only doctor. Older control databases without App tables retain their ordinary history.

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| Launch | Real controller, ACP provider, local shim, signed RPC, and HTTP invocation |
| Credentials | Bound, unbound, and revoked references absent from provider and shim environments |
| Policy | Undeclared tool, missing scope, invalid input, schema drift, and ungranted surfaces denied |
| Result | Whole-result bounds, structured output validation, unknown effect without resend |
| Version | Published and rebound version does not alter an existing activation |
| Revocation | App, Team grant, and binding denial against opened sessions and idle streams |
| Late fact | Admitted write completion remains durable after revocation |
| Projection | Team-scoped redacted Cards and retained pinned App/version in history and doctor |
| Compatibility | Existing Mem discovery, runtime launch, stream, and journal regressions |

## Operational Notes

An unavailable endpoint is not evidence that an earlier write was rejected. Operators should use
the shared journal and upstream reconciliation instead of retrying uncertain effects blindly.

## Open Risks

Authority and namespace are administrator assertions about the external service. Reusing one
external namespace under a deliberately different declared authority is outside automatic endpoint
verification. Upstream services must enforce the authenticated identity and scopes they accept.

## Source Journals

- [App runtime integration](../journal/2026-09-18-app-runtime.md)
- [App result validation](../journal/2026-09-18-app-result-validation.md)
