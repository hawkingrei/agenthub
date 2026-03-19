# Distributed Node Architecture

## Problem

AgentHub needs a distributed node architecture that works for two very different horizons:

- the first delivery window should stay simple because the initial cluster size is small and operational complexity must stay low
- later deliveries must scale to many nodes, large team broadcasts, and a stricter zero-trust model without replacing the whole transport stack

The architecture therefore needs a phased plan that keeps today's encrypted gRPC control/data plane, introduces gossip only where it adds value, and preserves a clean migration path from a single cluster-wide key to per-node identities. A coarse `phase 1/2/3` split is not sufficient because transport, broadcast, membership, scale, and identity do not need to evolve at the same pace.

## Scope

- define a phased architecture for `agenthub` and `agent node` communication
- define the control plane, membership plane, data plane, and audit plane boundaries
- define how `leader -> 100+ team members` broadcast should fan out through nodes
- define when gossip is required and what it is allowed to carry
- define the migration path from a cluster-wide shared key to zero-trust per-node identity
- define validation targets for phase 1, scale-out, and security hardening

## Non-Goals

- implementing full production-grade certificate rotation in phase 1
- replacing gRPC mailbox/control transport with a gossip-native payload transport
- removing the central AgentHub authority from bootstrap, policy, and audit flows
- designing a fully decentralized consensus system for team state

## Architecture

### Core Principles

- business payloads move over authenticated `gRPC`, not gossip
- gossip is a metadata plane for membership, health, and routing hints
- broadcast fanout is node-scoped before it is member-scoped
- the control plane remains authoritative even when the membership plane is eventually decentralized
- every phase must preserve a migration path toward stricter zero-trust identity
- each phase must have a clear exit criterion so the next phase can start without re-opening protocol basics

### Plane Separation

- `Control plane`
  - authoritative node registry
  - bootstrap, policy, placement constraints, audit trail
  - credential issuance and future credential revocation
- `Membership plane`
  - gossip-based membership, health, load, capability, and topology hints
  - non-authoritative routing inputs only
- `Data plane`
  - `node <-> node` mailbox, agent control, and broadcast envelopes over `gRPC`
  - request/response and future streaming transport for high fanout
- `Audit plane`
  - one persisted broadcast intent
  - per-node fanout state
  - aggregated acknowledgement and retry state

### Current Implementation Status

As of `2026-03-19`, phases `0` and `1` are implemented in the main codebase:

- phase 0 abstraction freeze now has concrete runtime surfaces in:
  - `src/internal/p2p.rs`
  - `src/internal/auth.rs`
  - `src/agent/manager.rs`
- phase 1 shared-key direct transport now carries forward-compatible metadata through:
  - internal auth claims and node credential issuance
  - internal gRPC actor send/list/ack wire fields
  - remote relay envelope metadata
  - blackbox and in-process p2p integration coverage

One implementation boundary is intentionally explicit:

- destination-local mailbox delivery still keeps `to_peer_id = main` so inbox lookup semantics remain local
- source node identity is preserved on delivered messages via `from_peer_id`
- explicit `target_node_id` continues to live in route/envelope metadata instead of overloading the destination-local mailbox partition key

### Phase Breakdown

#### Phase 0: Protocol And Abstraction Freeze

Goal:

- freeze the envelope and abstraction shapes that later phases will depend on

Deliverables:

- stable protocol fields for node-to-node and broadcast traffic
- explicit interfaces for:
  - `MembershipView`
  - `CredentialProvider`
  - `BroadcastPlanner`
  - `P2PTransport`
- explicit rule that `gossip` never becomes the mailbox payload transport

Exit Criteria:

- later phases can add implementation detail without changing the base envelope structure
- a reviewer can point to one canonical list of required fields and runtime abstractions

#### Phase 1: Small-Cluster Secure Direct P2P

Goal:

- ship a small-cluster baseline with low operational overhead

Deliverables:

- one cluster-wide shared signing key is allowed
- transport still uses `TLS/mTLS`
- direct authenticated `gRPC` for `node <-> node` and `node <-> AgentHub`
- AgentHub remains the source of truth for node registry and policy
- node-local persistence stays local

Exit Criteria:

- 2-node p2p mailbox/control works reliably
- retry, acknowledgement, and idempotency semantics are stable
- the shared-key simplification does not remove forward-compatible identity fields

#### Phase 2: Node-Scoped Broadcast Fanout

Goal:

- make `leader -> 100+ team members` practical without `100+` cross-node unicasts

Deliverables:

- one persisted `BroadcastIntent`
- audience resolution by member and `target_node_id`
- one `NodeBroadcastEnvelope` per target node
- local per-member fanout on the destination node
- one `NodeBroadcastAck` per destination node

Exit Criteria:

- cross-node broadcast cost is proportional to target node count instead of member count
- multiple members on the same node do not cause duplicate cross-node payload sends

#### Phase 3: Centralized MembershipView

Goal:

- decouple routing from direct registry/database reads before gossip is introduced

Deliverables:

- a `MembershipView` implementation backed by:
  - AgentHub node registry
  - heartbeat freshness
  - local routing cache
- routing and placement decisions read from `MembershipView`, not ad hoc storage calls

Exit Criteria:

- `BroadcastPlanner` and placement logic do not need to know whether membership came from a registry or from gossip
- gossip can be introduced later without reworking the broadcast or transport layers

#### Phase 4: Authenticated Gossip Membership

Goal:

- add membership propagation and health dissemination for larger node counts

Deliverables:

- authenticated gossip for:
  - node liveness
  - load hints
  - capability/version advertisement
  - zone/rack locality hints
  - compact broadcast descriptors for coordination
- a merge policy between centralized registry truth and gossip observations

Exit Criteria:

- membership converges fast enough for scale-out routing
- gossip remains advisory and does not become an authorization source

#### Phase 5: Scale Fanout And Performance

Goal:

- keep latency and resource usage acceptable as node count and broadcast volume rise

Deliverables:

- pooled `gRPC` channels
- batch node envelopes or streaming delivery where useful
- per-peer inflight limits
- retry budgets and jittered backoff
- node-aware or zone-aware fanout planning
- `gossip descriptor + gRPC payload` for large fanout coordination when needed

Exit Criteria:

- large fanout does not create connection storms or repeated full-payload floods
- hotspot nodes can be detected and avoided with load-aware routing

#### Phase 6: Zero-Trust Identity Upgrade

Goal:

- replace the shared signing key with per-node identity and short-lived scoped credentials

Deliverables:

- per-node identities
- short-lived and revocable scoped credentials
- identity-bound gossip advertisements
- strict request-time validation of:
  - node identity
  - audience
  - scope
  - lifetime

Exit Criteria:

- the cluster-wide shared key is no longer the primary trust root
- replayed, stale, or spoofed node communications can be rejected without trusting shared secrets

### Broadcast Model For 100+ Team Members

The leader should not emit one remote mailbox send per member.

Recommended flow:

1. persist one `BroadcastIntent`
2. resolve target members for the team/run/conversation
3. group members by `target_node_id`
4. send one `NodeBroadcastEnvelope` per target node over `gRPC`
5. let each target node perform local per-member fanout
6. aggregate local delivery state into one `NodeBroadcastAck`
7. let the control plane reconcile global completion from per-node acknowledgements

This reduces cross-node work from `O(member_count)` to `O(target_node_count + local_fanout)`.

### Gossip Usage Rules

Allowed uses:

- membership discovery
- failure detection
- load and capacity hints
- capability/version dissemination
- small broadcast descriptors or invalidation markers

Disallowed uses:

- direct mailbox payload transport
- direct agent-control payload transport
- ordering-sensitive delivery
- authoritative authorization
- large binary or structured task payload replication

### Protocol Objects

The phased design should stabilize these protocol objects early:

- `BroadcastIntent`
  - one logical broadcast initiated by leader/control plane
- `NodeBroadcastEnvelope`
  - one node-scoped delivery unit containing audience or audience filters for a single target node
- `NodeBroadcastAck`
  - one node-scoped acknowledgement and summary
- `BroadcastDescriptor`
  - a compact gossip-safe descriptor containing metadata only, never the main payload

### Forward-Compatible Required Fields

Even in phase 1, messages and envelopes should carry fields that keep later migration possible:

- `cluster_id`
- `source_node_id`
- `target_node_id`
- `broadcast_id`
- `correlation_id`
- `idempotency_key`
- `scope`
- `issued_at`
- `expires_at`
- `audience`
- `kid`
- `payload_digest`

### Phase Dependencies

- Phase 0 must land before the other phases harden around unstable protocol shapes
- Phase 1 is the minimum viable distributed node delivery
- Phase 2 depends on Phase 1 because broadcast still rides the same authenticated `gRPC` data plane
- Phase 3 should follow Phase 2 so the membership abstraction is in place before gossip is introduced
- Phase 4 depends on Phase 3 because gossip should plug into `MembershipView`, not bypass it
- Phase 5 depends on Phase 4 because scale tuning needs live membership and load hints
- Phase 6 can begin incrementally after Phase 0, but it is operationally safer after Phase 4 and Phase 5 have stabilized the routing model

## Contracts

- `node <-> node` mailbox and agent-control business traffic must use authenticated `gRPC`
- `gossip` is a metadata plane only and must not carry mailbox business payloads
- the control plane remains authoritative for registry, bootstrap, policy, and audit
- phase 1 may use a cluster-wide shared signing key, but protocol shapes must remain compatible with per-node identity in later phases
- cross-node broadcast must aggregate by target node before local per-member fanout
- nodes may use gossip-derived membership and load information only as routing hints, never as a standalone trust source
- phase boundaries are driven by protocol stability and operational readiness, not by implementation convenience

## Validation Matrix

Phase 0:

- protocol field review for `P2PTransport` and broadcast envelopes
- abstraction review for `MembershipView`, `CredentialProvider`, and `BroadcastPlanner`

Phase 1:

- 2-node blackbox `gRPC` p2p pipeline
- point-to-point retry/idempotency coverage

Phase 2:

- node-scoped broadcast fanout test with multiple members on the same target node
- retry/idempotency test for duplicate `NodeBroadcastEnvelope`
- fanout aggregation correctness test across mixed target nodes

Phase 3:

- `MembershipView` fallback tests from registry and heartbeat data
- planner tests that do not depend on a concrete membership backend

Phase 4:

- 8-node and 16-node membership propagation soak
- node flap and partial partition tests
- broadcast descriptor propagation tests

Phase 5:

- load-aware fanout routing tests
- inflight limit and retry-budget tests
- hotspot avoidance and large-fanout latency benchmarks

Phase 6:

- per-node credential rotation
- node revoke / deny-list propagation
- spoofed membership advertisement rejection
- replayed credential rejection

Recommended implementation commands should be added as each phase lands; phase 1 currently depends on:

```bash
cargo test --locked --test distributed_p2p_pipeline -- --nocapture
```

## Operational Notes

- phase 1 intentionally trades strict zero trust for lower bootstrap complexity
- that simplification is acceptable only if message/envelope schemas already include forward-compatible identity and scope fields
- gossip should be introduced only when node count and fanout patterns justify it
- when large fanout appears, the preferred model is `gossip descriptor + gRPC payload`, not `gossip payload flood`
- debugging and auditability must remain possible across every phase; this is another reason not to use gossip as the primary mailbox transport
- splitting the roadmap into phases `0-6` is intentional so broadcast, membership, scale, and identity can progress independently without forcing premature zero-trust or premature gossip rollout

## Open Risks

- phase 1 shared-key deployments can blur trust boundaries if request scopes are not enforced tightly enough
- phase 2 fanout can still overload hotspot nodes if node grouping is correct but local fanout controls are weak
- phase 3 can become a thin wrapper with little value if planners keep bypassing `MembershipView`
- phase 4 gossip-based load hints can become stale and cause suboptimal routing under rapid cluster churn
- phase 6 identity migration will still require a careful bootstrap and revocation design

## Source Journals

- `docs/journal/2026-03-19-distributed-node-architecture.md`
