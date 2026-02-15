# Internal gRPC Zero-Trust Foundation

## Summary

Introduce an internal gRPC control-plane baseline for Team orchestration with
zero-trust authorization defaults and encrypted transport by default.

## Background

Current Team orchestration APIs are HTTP-first and exposed on the same API
surface as user-facing operations. We need an internal RPC channel that is
better aligned with node-to-node orchestration workflows and security policy.

## Scope

- `Cargo.toml`
- `build.rs`
- `proto/internal/v1/team.proto`
- `src/config.rs`
- `src/internal/mod.rs`
- `src/internal/auth.rs`
- `src/internal/service.rs`
- `src/internal/tls.rs`
- `src/main.rs`
- `docs/todo.md`

## Key Decisions

1. Add `TeamInternalControl` gRPC service with 5 internal RPCs:
   - `SendActorMessage`
   - `ListActorInbox`
   - `AckActorMessage`
   - `TransitionStep`
   - `IssueNodeCredential`
2. Add explicit internal authz model:
   - bearer token with signed custom claims (`role`, `actor_id`, `run_id`,
     `permissions`);
   - action-based permission checks with default deny.
3. Enforce worker-role constraints as zero-trust baseline:
   - worker token can only operate on its own `actor_id`;
   - worker token cannot call `TransitionStep`.
4. Add internal transport security modes:
   - `tls` (default): auto-generate CA + server cert + bootstrap client cert
     under `~/.agenthub/internal-grpc/`;
   - `mtls`: enable client cert verification (`client_ca_root`) with
     `client_auth_optional=true` so bootstrap can issue first credentials;
   - `disabled`: local/CI convenience mode.
5. Auto-bootstrap internal auth secret and join token:
   - if `internal_grpc.auth.shared_secret` is unset, generate and persist
     `auth_secret.txt` in cert dir.
   - if `internal_grpc.bootstrap.token` is unset, generate and persist
     `bootstrap_token.txt` in cert dir.
6. Add node self-join bootstrap flow:
   - `IssueNodeCredential` validates `x-agenthub-bootstrap-token`;
   - issues scoped access token (`leader` / `worker`) with default-deny
     permissions;
   - worker bootstrap requires both `actor_id` and `run_id`;
   - returns mTLS client bundle (`client cert/key + ca cert`) when security
     mode is `tls`/`mtls`.
7. Keep user-facing HTTP unchanged:
   - internal gRPC listener is optional and enabled via config.

## Configuration

```toml
[internal_grpc]
enabled = true
listen = "127.0.0.1:50051"

[internal_grpc.security]
mode = "mtls" # mtls | tls | disabled
cert_dir = "~/.agenthub/internal-grpc"

[internal_grpc.auth]
# optional: auto-generated file if omitted
shared_secret = "replace-me"
issuer = "agenthub"
audience = "agenthub-internal"

[internal_grpc.bootstrap]
# optional: auto-generated file if omitted
token = "replace-me-join-token"
```

## Cloudflare Deployment Note

When Cloudflare proxy (orange-cloud) is enabled, TLS is terminated at the
Cloudflare edge first. For this topology, treat mTLS as two independent links:

1. Client node -> Cloudflare edge:
   - use Cloudflare mTLS / client certificate policy for node identity.
2. Cloudflare edge -> AgentHub origin:
   - use Authenticated Origin Pulls (or equivalent origin mTLS) so AgentHub
     only accepts traffic from trusted Cloudflare identities.

This means source-side "single-hop end-to-end mTLS from node to AgentHub" is
not available through standard L7 proxying. If strict origin-side client cert
validation is required, deploy internal gRPC on a private direct path
(non-proxied / private network listener).

## Validation

```bash
cargo test internal:: -- --nocapture
cargo test internal_grpc_defaults_are_stable
```

## Follow-ups

- Add same-port HTTP+gRPC multiplexing.
- Add production-grade workload identity and mTLS issuance/rotation
  (SPIFFE/SPIRE or equivalent).
- Add internal gRPC client bootstrap helper + E2E join/renew tests.
