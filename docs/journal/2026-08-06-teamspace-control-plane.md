# Teamspace Control Plane

## Summary

Teamspace now has a separate multi-user control plane for membership, invites, execution claims,
and audit evidence while keeping runtime agent member identities unchanged.

## Background

Existing Team ownership was a single local-account field. Shared Team visibility and execution
needed explicit human membership without making a Task or Step jointly executable.

## Scope

- Add owner-created, single-use, digest-only Teamspace invites and local-account registration.
- Grant Team visibility through active membership and support audited member revocation.
- Enforce one active generation-fenced Task or Step execution claim and explicit owner handoff.
- Apply Teamspace role checks to configuration, runtime controls, invite creation, work planning,
  and run creation.

## Key Decisions

- Teamspace membership remains separate from the runtime `member_id` used for agent execution.
- Invite tokens are URL fragments, cleared by the browser, and submitted only in request bodies.
- A handoff releases the previous Task claim and requires the successor to claim a new generation.
- Revocation preserves history and audit records but immediately removes future Teamspace access.

## Validation

```bash
cargo test teamspace_invite_grants_member_visibility_once --lib
cargo test teamspace_invite_is_single_use_and_task_claim_is_single_owner --lib
cargo check -q
cd web && npm test -- --run src/pages/join_page.test.tsx src/api.test.ts
cd web && npm run build
git diff --check
```

## Follow-Ups

- Add the owner-facing member and invite management surface to the Team workspace.
- Map human membership removal to active runtime work so affected work is explicitly handed off or
  moved to `waiting`.
- Add browser coverage for invite creation, existing-account acceptance, and role-gated controls.
