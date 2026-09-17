# App Registry Storage

## Problem

App versions and permissions must survive process restarts without changing the meaning of an
already admitted tool invocation or reviving a revoked grant.

## Scope

The initial storage foundation implements bounded manifests, immutable app identity and numbered
versions, Team approval, explicit member bindings, activation pins, and durable call admission.
The [App integration contract](app-tool-registration.md) owns the complete runtime target.

## Non-Goals

- Public management routes, provider mounts, Cards, or signed events in this storage checkpoint.
- Automatic latest-version tracking, remote credential delivery, or local command hosting.
- Inferring successful external effects from a grant, launch, or process exit.

## Architecture

`agenthub-agent-domain::app_tools` validates manifests and provisioned connection descriptions.
`agenthub-db::app_registry` stores registration, versions, grants, bindings, and activation snapshots
in the existing control database. The production database initializer applies the additive migration.
No legacy member receives an app or execution authority during migration.

## Contracts

- Only instance configuration may provision an immutable endpoint, credential environment reference,
  authority, and namespace. The endpoint permits HTTPS or loopback HTTP without URL credentials,
  query, or fragment. Credentials are resolved daemon-side; safe records omit connection details.
- Authority/namespace pairs remain unique even after revocation. Another display name or app id
  cannot become an alias for the same external effects. App ownership is immutable in this version.
- Manifest schema version 1 uses JSON Schema 2020-12 object declarations. Input and optional output
  declarations are bounded; external HTTP/file references are not fetched. Regex validation uses a
  bounded linear-time engine. Arguments remain native JSON and are validated without rewriting.
- Manifest size is at most 256 KiB, with at most 64 tools and 64 declared scopes. Each tool requires
  nonempty declared scopes and an explicit read-only, non-idempotent, or stable-identity replay policy.
  Stable identity paths contain at most eight required object properties ending in a string.
  Argument/output validation is bounded to 1 MiB, 64 levels, and 32,768 JSON nodes.
- Publication creates a new immutable numbered version under the app owner's observed revision;
  concurrent publishers cannot overwrite a version. The initial limit is 1,024 versions per app.
- App-owner approval grants a Team a scope subset. Team ownership is additionally required at the
  HTTP boundary. Approval alone grants no member any tool. Team owners must explicitly bind each
  member to a version within both the Team grant and that manifest's scopes.
- At most 64 active Team grants and 16 active member bindings are retained per corresponding scope.
  Management lists use bounded keyset reads. Revoked records remain available for audit.
- Optimistic revision and authorization epoch are separate. Selecting another version with unchanged
  permissions increments the revision without revoking existing activation pins. Changing permissions,
  revoking, or restoring a revoked grant advances its epoch; old pins cannot regain authority.
- The first starting generation records the activation's complete app selection, including an empty
  selection. Pins contain the effective intersection of Team and member scopes, version, grant and
  binding revisions, authorization epochs, and the generation that originally selected them.
  Startup retries retain this selection while the ordinary loop lease fences previous generations.
- Each proxy admission must call the durable authorization helper after authenticating the executor.
  The helper verifies the current loop phase/lease, app/grant/binding revocation, permission subsets,
  and pinned authorization epochs in one read transaction. Only bootstrap requests may use the
  established starting-phase allowance. A previously admitted call may finish factually; later calls
  are denied. Historical pins remain readable and never confer authority by themselves.

## Validation Matrix

| Boundary | Focused evidence |
| --- | --- |
| Schemas and scope intersection | Domain declaration/argument/output checks, bounded schemas, denied external references |
| Credential references | URL transport constraints and payload-free validation failures |
| Publication | Concurrent revision updates, immutable historical versions, file reopen |
| Registration revocation | Retained history and rejected authority alias registration |
| Team/member grants | Membership, approved scope subsets, explicit versions, revision conflicts |
| Epoch changes | Version-only updates, narrowed permissions, revoke/rebind and revoke/reapprove |
| Activation snapshots | Empty selections, startup retries, generation fencing, immutable versions |
| Durable admission | Binding/Team/app revocation, reload persistence, expired leases and foreign executors |

## Operational Notes

Storage methods are daemon interfaces, not authentication endpoints. Production management routes
must enforce instance, app-owner, and Team-owner checks before mutation. Runtime proxy wiring must
use durable admission for every request, including sessions opened before revocation.

## Open Risks

Public API, launch/preflight, shared proxy enforcement, safe discovery, and trace projections are
the remaining integration work for slice 14. App event ingress belongs to slice 15.

## Source Journals

- [App registration storage checkpoint](../journal/2026-09-17-app-registry-storage.md)
