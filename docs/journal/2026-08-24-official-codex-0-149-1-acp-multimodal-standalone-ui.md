# Official Codex 0.149.1 ACP Multimodal And Standalone UI

## Summary

AgentHub now pins its Codex ACP runtime to the official `openai/codex` `0.149.1`
source commit `ff29a44391deccde0aba0f8390337d7f3c319ea4`. The ACP bridge tracks the
upstream app-server protocol additions for asynchronous agent delivery, collaboration, subagent
activity, and image generation. The standalone ACP workbench now exposes that runtime state and
supports bounded image input and output.

No downstream Codex fork is used by the Cargo or Bazel Codex source pins.

## Baseline

- Cargo Codex crates and Bazel `codex_src` previously did not represent the requested official
  release boundary.
- Upstream history types moved into `codex-history`, user prompt submission moved from
  `Op::UserInput` to `Op::TurnInput`, and app-server agent messages gained delivery metadata.
- Codex collaboration and subagent notifications were available upstream but were not represented
  consistently in AgentHub's ACP event stream or standalone UI.
- The standalone ACP composer supported text only, and image-generation payloads were not rendered
  as image content.

## Implementation

### Official Codex Source

- All direct Codex Cargo git dependencies now use `https://github.com/openai/codex` at
  `ff29a44391deccde0aba0f8390337d7f3c319ea4`.
- `MODULE.bazel` uses the same official repository and commit.
- `Cargo.lock` was refreshed from that source, and the former Codex-specific crate patches were
  removed where they conflicted with the official release graph.
- Runtime and test code now use `codex-history` envelopes and `Op::TurnInput` without weakening the
  existing history-repair, prompt-steering, approval, or configuration tests.

### CI Compatibility Repairs

- The official `codex-code-mode-protocol` build script now honors an explicit `PROTOC` executable
  before falling back to its vendored Cargo executable. Bazel supplies the hermetic protobuf
  compiler through crate-universe build-script metadata, so the generated protocol remains on the
  official Codex source graph without depending on an absolute path from another sandbox.
- The rollout-history repair loop uses an equivalent `let ... else` shape that satisfies the
  workspace `single_match` lint without changing which response items are repaired.
- The Team ACP duplicate-send test now follows the successful-send contract: the cleared composer
  remains disabled until a new prompt is entered, then duplicate triggers are still suppressed
  while that follow-up prompt is in flight.
- The Team task handoff CAS regression now prepares the terminal update before either concurrent
  writer starts. It exercises the complete production transaction through an internal helper, so a
  stale update still loses while a valid handoff-then-update serial order is no longer misclassified
  as a concurrency failure under coverage instrumentation.

### Updated Codex Capabilities Through ACP

- Asynchronous Codex agent messages retain `delivery = async` metadata through the ACP event path
  and render with an explicit background-delivery label.
- Collaboration tool calls and subagent lifecycle activity become deterministic ACP tool calls with
  `agenthub.kind` metadata.
- Codex runtime profiles now preserve the upstream `xhigh`, `max`, and `ultra` reasoning efforts
  instead of reducing AgentHub's `max` profile to `high`. Codex-only levels are rejected for Claude.
- Codex subagents remain provider-internal ACP activity. They do not create AgentHub Team members or
  mutate Team control-plane state.
- Repeated subagent events update one card per Codex thread, and active subagent cards are excluded
  from generic stale-tool settlement.
- Image-generation lifecycle events become ACP tool calls whose terminal content contains the image
  and optional saved-resource link without copying image base64 into raw debug output.

### Local ACP Multimodal Input

- The agent input route accepts text, images, or both for local ACP runtimes.
- Image attachments are converted to ACP `ImageContent` blocks and then to Codex image user input.
- Image-only prompts omit an empty text block.
- Backend validation allows PNG, JPEG, WebP, and GIF, checks declared MIME type against file
  signatures, and enforces four images, 5 MiB per image, and 10 MiB total decoded bytes.
- The input route has a 16 MiB request-body limit for JSON and base64 overhead.
- Remote agents and stdin-only runtimes reject multimodal input explicitly instead of dropping it.
- Normalized user events retain bounded attachment data so history and SSE replay can reconstruct
  the conversation after a page reload.

### Standalone ACP Workbench

- The composer supports file selection, clipboard paste, drag-and-drop, thumbnail preview, removal,
  client-side validation, and image-only sends.
- Failed sends and retries retain image attachments; successful sends clear text and images
  together.
- The workbench header now shows run state, model, live reasoning effort, effective permission mode,
  and active/total Codex subagent counts. Live ACP values override persisted startup defaults.
- Message and tool bubbles render only allowlisted raster image formats from safe `data:` or HTTP(S)
  sources. SVG and local-file URI rendering remain blocked.

## Validation Evidence

The focused checks cover the changed dependency, protocol, API, and UI boundaries:

```bash
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check -p agenthub-codex-acp-runtime -p agenthub-acp-adapter -p agenthub-acp -p agenthub
cargo test -p agenthub-codex-acp-runtime
cargo test -p agenthub-acp-adapter
cargo test -p agenthub-acp prompt_
cargo test -p agenthub input_image_validation
cargo test -p agenthub-config normalize_optional_thinking_level
cargo test -p agenthub codex_reasoning_effort_maps_thinking_levels
cargo test -p agenthub create_agent_route_
cargo test --locked -p agenthub concurrent_terminal_status_update_and_handoff_do_not_both_apply -- --nocapture
npm run lint
npx tsc --noEmit
npm run build
npm run test:coverage
npx vitest run src/acp.test.ts src/components/input_dock.test.tsx src/components/acp_media_gallery.test.tsx src/agents_workbench.test.tsx src/components/use_agents_workbench_panel.test.tsx src/api.test.ts src/create_agent_modal.test.tsx src/pages/team/team_management_modals.test.tsx
```

The Codex runtime suite contains 135 passing tests, and the provider adapter suite contains eight
passing tests. The multimodal ACP prompt subset contains seven passing tests, the backend image
validator contains two passing tests, and the focused web matrix contains 82 passing tests across
eight files. The extended reasoning checks add one config normalization test, one Codex mapping
test, and 13 create-agent route tests.

The CI repair follow-up also passed the full Web coverage matrix: 1,503 tests across 162 core test
files plus 30 Team smoke tests. The production build, ESLint, TypeScript typecheck, workspace
all-targets Clippy, and formatting checks completed successfully.

The affected Bazel binaries and test targets passed with Bazel 9.2.0:

```bash
bazel build //agenthub-codex-acp:agenthub_codex_acp_bin //crates/agenthub-acp-adapter:agenthub_acp_adapter_bin
bazel test //agenthub-codex-acp:agenthub_codex_acp_tests //crates/agenthub-acp-adapter:agenthub_acp_adapter_tests
bazel test --nocache_test_results --runs_per_test=20 --test_arg=concurrent_terminal_status_update_and_handoff_do_not_both_apply //:agenthub_unit_tests
```

The build completed 4,082 actions and both ACP tests passed. The deterministic Team task CAS
regression also passed 20 independent Bazel runs. The commands used an isolated local output root
and repository/disk caches to avoid a capacity-limited `/tmp` mount; no project or user Bazel
configuration was changed.

Chrome DevTools MCP was not available in this environment, so this checkpoint does not claim
before/after browser evidence. The DOM-level interaction matrix and production web build are the
fallback evidence for this rollout.

## Risks And Follow-Ups

- The official Codex graph reintroduces older or parallel versions of several transitive crates
  compared with the previous downstream pin. The resolved runtime graph currently includes
  `gix 0.81.0`, `gix-fs 0.19.2`, `gix-pack 0.68.0`, `hickory-proto 0.25.2`,
  `jsonwebtoken 9.3.1`, `opentelemetry_sdk 0.31.0`, and `tar 0.4.45`, which match previously
  fixed Dependabot advisory ranges (`GHSA-fr8x-3vfx-f45h`, `GHSA-pg4w-g64p-qwhj`,
  `GHSA-f26g-jm89-4g65`, `GHSA-p3hw-mv63-rf9w`, `GHSA-f89h-2fjh-2r9q`,
  `GHSA-x494-mj8g-cj27`, `GHSA-q2qq-hmj6-3wpp`, `GHSA-3v94-mw7p-v465`,
  `GHSA-h395-gr6q-cpjc`, `GHSA-w9wp-h8wv-79jx`, and `GHSA-3pv8-6f4r-ffg2`).
  A direct `cargo update -p tar --precise 0.4.46` check cannot resolve because official
  `codex-core-plugins 0.149.1` pins `tar = "=0.4.45"`; the other affected lines also require
  upstream-compatible dependency changes. Keep the official source boundary and address these
  findings in a newer official release rather than restoring a Codex fork.
- Inline base64 user-event persistence is intentionally bounded but increases SQLite and SSE replay
  cost. Migrate attachments to owner-scoped object references and replay-safe thumbnails before
  raising the current limits.
- Run an authenticated real-browser pass for image selection, paste, drag/drop, reload replay,
  generated-image rendering, asynchronous delivery, and concurrent Codex subagent activity.
