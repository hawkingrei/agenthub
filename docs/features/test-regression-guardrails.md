# Test Regression Guardrails

## Problem

State migration and message-routing regressions can be locally invisible when a test fixture omits a
table, index, or constraint that the production schema requires. A passing focused test then proves
only the incomplete fixture, not the production behavior.

## Scope

- Define the minimum regression evidence for changes to durable state transitions and message routing.
- Define when a test fixture must reuse the production schema initializer or verify equivalence with it.
- Apply to SQLite authority tables, durable outboxes, checkpoints, routing state, and their API and
  manager tests.

## Non-Goals

- Replacing every focused fixture with a full application bootstrap.
- Requiring an end-to-end test for every state-transition or routing change.
- Making test-only seed data mirror unrelated production data.

## Architecture

Focused tests may use reduced seed data, but their schema is part of the test contract. A fixture that
exercises production persistence must either:

1. initialize through the production schema/migration entrypoint; or
2. explicitly create every table, index, trigger, and constraint used by the exercised code path.

The second option is acceptable only when full initialization would obscure the behavior under test or
make the test materially slower. It must name the protected schema objects in the test or nearby helper.

## Contracts

### 1) Protected Object Declaration

Every change to a durable state transition or message-routing path must name the object protected from
regression. The declaration belongs in the test name, a concise test comment, or the PR validation
summary and includes:

- the authority object or transition, such as `team_actor_messages.status` or a task ownership claim;
- the invariant, such as no duplicate delivery, monotonic checkpoint, or unauthorized transition is
  rejected;
- the focused automated test that proves it.

The test must cover the failing boundary and its closest valid neighbor. For a bugfix, the failing shape
must be reproducible before the fix or otherwise be represented by a targeted negative assertion.

### 2) Schema Fixture Consistency

Any test that calls code touching durable tables must keep its fixture consistent with the production
schema for the objects on that path.

- A new table, index, trigger, or required column must update each reduced fixture that exercises it in
  the same change.
- A fixture may not silently substitute a missing table or weaker constraint for the production object.
- Migration and backfill tests must exercise the real migration entrypoint or include an explicit
  legacy-schema fixture plus the post-migration assertion.
- New durable objects need one focused test that fails when the object is absent from the fixture,
  typically by executing the production path rather than only querying the object directly.

### 3) Routing And Transition Boundaries

For message-routing and state-transition changes, tests must assert the authority result rather than
only a response payload. Relevant assertions include persisted status, ownership, routing target,
idempotency key, checkpoint, or durable outbox state.

Tests should keep transport mocks at the edge. They must not bypass the authorization, scope, or
transition resolver that the production route uses.

## Validation Matrix

| Change | Required focused evidence |
| --- | --- |
| New durable schema object | Production-path test using a fixture that contains the object; absent-object failure is covered by the fixture contract. |
| State transition | Invalid transition is rejected and the closest valid transition persists the expected authority state. |
| Message route | Canonical target/scope is persisted, duplicate/replay behavior is asserted, and the route cannot bypass authorization. |
| Backfill or recovery | Resume checkpoint and idempotency are asserted across an interrupted or repeated execution. |
| Fixture helper change | The helper is checked through at least one representative production path. |

## Operational Notes

- Prefer a shared production initializer when a fixture already needs many authority tables.
- Keep deliberately reduced fixtures small, but colocate their schema declarations and name the paths
  they support.
- Treat a missing-table error in a broader test as a fixture-contract regression, not as a reason to
  weaken the production path.
- CI can later enforce a shrink-only ratchet for known fixture drift; this contract is the prerequisite
  for defining that ratchet without false positives.

## Open Risks

- Handwritten fixtures can still drift when they cover many unrelated tables; migrate those fixtures to
  a shared initializer incrementally when they next change.
- Full schema initialization can make narrowly-scoped tests slower, so it should not be mandatory when
  an explicit reduced fixture is clearer and complete for its path.

## Source Journals

- [docs/journal/2026-03-10-pr-118-ci-fixture-alignment.md](../journal/2026-03-10-pr-118-ci-fixture-alignment.md)
- [docs/journal/2026-07-13-message-body-store-phase1-dual-write.md](../journal/2026-07-13-message-body-store-phase1-dual-write.md)
