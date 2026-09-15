# Scoped Mem Context Bootstrap

## Summary

Add one scoped context-lens read to each local loop activation, including provider resume. Preserve
the attributed markdown as data, classify unavailable knowledge separately from invalid authority,
and retain independent local task progress. Selected learning uses native discovery and preserves
task, originating activation, and artifact provenance. Slice 10 publication and CI remain open.

## Background

Slice 9 supplies the credential-isolating MCP proxy and durable operation journal. Previously a
temporary failure of the membership probe prevented all activation work, and launch did not recover
the context lens. The upstream contract was inspected in the local Mem source at
`f2d52afa86e5f17895f62d9d94608097f5581f8b`, including Cloud's MCP 2025-06-18 initialization,
`read_context_bundle` declaration, scoped authorization, and markdown JSON result.

## Scope

- Daemon-side authorization failure classification and configuration fingerprints.
- A bounded consumer of the existing proxy with normal lifecycle fences and journal admission.
- One attributed-data block or visible knowledge failure in entry prompt version `loop-entry-v4`.
- Fenced, immutable, body-free context outcome events in the existing activation event table.
- Native selected-learning output and recovery contracts, including readable legacy note inputs.

## Key Decisions

- Do not mount an unverified upstream binding when allowing independent local work.
- Preserve dynamic schemas and native responses; the bootstrap is a consumer of those contracts.
- Assign trusted read-only replay to the context lens and native memory/working-memory/thread/source
  retrieval contracts. Unknown tools and memory writes do not gain replay authority from annotations.
- Reject oversized or wrong-scope context in full. Keep successful markdown byte-for-byte.
- Bound waiting without canceling an already admitted journaled exchange; retain its daemon guard
  until factual settlement and session retirement.
- Prompt review classification: runtime recovery pointer, data boundary, and selected-learning
  output contract. Entry text grows only for configured Mem activations. No private native tool
  recipes or role skill entrypoints are added; role integration remains slice 11.
- Keep original selected provenance during recovery. Cloud's declared `source_grounding` is a
  nullable string, and its tool profile does not promise caller-ID upsert. When the field is absent,
  selected content carries the evidence references without extending the discovered schema.

## Validation

Focused coverage passes for this checkpoint:

```bash
cargo +1.96.0 clippy -p agenthub --all-targets --locked --offline -- -D warnings
cargo +1.96.0 test -p agenthub-db loop_mem_context --locked --offline
cargo +1.96.0 test -p agenthub --lib mcp_proxy:: --locked --offline
cargo +1.96.0 test -p agenthub --lib agent::manager::loop_launch:: --locked --offline
cargo +1.96.0 test -p agenthub --lib mcp_ --locked --offline
```

Results: all-target Clippy, one database event regression, nine proxy tests, seven launch tests,
and 38 MCP regression tests pass. The launch and MCP selections overlap; each includes two otherwise
ignored helpers invoked by parent process tests. The actual CLI was rebuilt for these tests.
Formatting, whitespace, and seven local documentation targets pass their checks.

The actual ACP fixture performs fresh/fresh/resume recovery with new attributed markdown each time,
then writes canonical task notes with authorization-probe failure, transport failure, a native tool
error, a missing tool, a foreign-space bundle, and a repeated discovery cursor. A tool without a
declared space property receives unchanged arguments. Discovery is paginated. A repeated cursor
is rejected by the shared proxy before reaching the consumer and is recorded as unavailable;
malformed or wrong-scope returned bundles are recorded as invalid. Failure wording does not imply
that every unsuccessful read was a temporary outage.

The existing crash regression first timed out during fixture schema creation in the disk-backed
temporary directory, before proxy initialization. With a private command-local `TMPDIR` under
`/dev/shm`, the same four-window crash test passes in 2.88 seconds and the full MCP selection passes
in 3.29 seconds. No production settings, timeouts, crash assertions, or Bazel configuration changed.
Other validation used the existing command-local Rust temporary directory and loopback proxy bypass.

## Deadline And Learning Validation

The consumer-deadline fixture holds an admitted upstream read beyond its consumer's deadline. The
consumer returns unavailable while independent task evidence remains writable and cleanup remains
fenced. Releasing the upstream records factual success before cleanup acquires its guard. The
activation retains its original unavailable event. The focused parent test passes for this boundary.

Additional fixtures exercise eager native initialization failure, selected learning through native
provenance or content, read recovery, and an applied write with no receipt across two activations.
The final launch selection passes eight parent/unit tests. Three additional child helpers run
through their parents (listed as ignored by the outer harness). Root all-target Clippy also passes. The owned
Codex adapter's existing optional MCP configuration is guarded explicitly; its focused build/test
is pending at this checkpoint. The pinned runtime validates only required servers
(`codex-mcp/src/connection_manager/required.rs` at `9085439`). Fake ACP behavior does not prove how
arbitrary external providers or language models handle tool outages or select learning.

## Follow-Ups

- Complete focused adapter validation and shared proxy regression checks.
- Publish the complete slice 10 PR and verify its CI before treating the slice as delivered.
- See [the canonical Mem contract](../features/nowledge-mem-mcp-proxy.md) and [active work](../todo.md).
