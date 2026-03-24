# Summary

Switch Bazel CI away from the deprecated GNU gold linker by making `lld`
available on the GitHub Actions runner before Bazel configures the local C/C++
toolchain.

# Why

Rust 1.94 warns when the final link step uses `-fuse-ld=gold`:

```text
warning: the gold linker is deprecated and has known bugs with Rust
```

In this repository, Bazel Rust targets inherit linker selection from the Bazel
C/C++ toolchain. The related `rules_rust` issue
[`bazelbuild/rules_rust#1114`](https://github.com/bazelbuild/rules_rust/issues/1114)
shows the same forwarding path from `cc_toolchain` into `rustc`.

For GitHub-hosted Linux runners, Bazel's local C/C++ toolchain prefers `lld`
when it is available and otherwise falls back to `gold`. Our Bazel workflow
installed the Rust and native library prerequisites, but not `lld`, so the CI
baseline was still selecting `gold`.

# What Changed

- Added `lld` to `.github/workflows/bazel.yml` system dependencies.
- Kept the fix scoped to CI runner provisioning instead of forcing a repository
  wide Rust linker override.
- Added a CI verification backlog item to `docs/todo.md` so the warning
  disappearance can be confirmed on both `push` and `pull_request`.

# Why This Approach

- It matches Bazel's native linker selection path instead of layering a custom
  linker policy on top of `rules_rust`.
- It avoids introducing a repo-wide `.bazelrc` override that could diverge from
  local developer environments or interact poorly with mixed Rust/C linking.
- It addresses the actual CI symptom source: `gold` is only chosen because
  `lld` is absent on the runner image.

# Validation

- `rustc /tmp/min.rs -o /tmp/min-gold -Clinker=gcc -Clink-arg=-fuse-ld=gold`
  reproduces the exact Rust gold-linker deprecation warning locally.
- Expected CI validation:
  - `bazel build //...`
  - `bazel test --test_output=errors //...`

Expected result:

- Bazel CI no longer emits the Rust gold-linker deprecation warning once the
  runner has `lld` available during local C/C++ toolchain detection.
