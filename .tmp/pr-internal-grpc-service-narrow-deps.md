## Summary

- replace `TeamInternalControlService`'s root-state dependency on `AppState` with an explicit `TeamInternalControlDeps` bundle
- limit that bundle to the four domains the internal gRPC control surface currently uses: `db`, `agents`, `teams`, and `acp_permissions`
- update bootstrap plus internal service/client tests to construct the narrow dependency bundle directly

## Why

- the previous `src/internal/service/{mod,rpc,helpers,tests}` split only moved files; the service still depended on full `AppState`
- that broad dependency kept the internal gRPC control surface tightly coupled to the root crate and blocked the remaining crate-split follow-up
- this keeps the runtime behavior and wire contract unchanged while making the dependency boundary explicit

## Testing

- `cargo test -p agenthub 'internal::service::tests::' -- --nocapture`
- `cargo test -p agenthub 'internal::client::tests::' -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `git diff --check`

## Notes

- `cargo fmt --all --check` still reports pre-existing unrelated formatting drift in `src/actor_cli/{parse,execute}.rs`; this PR keeps that noise out and only formats touched internal files
