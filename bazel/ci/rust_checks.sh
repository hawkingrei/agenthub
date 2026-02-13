#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${BUILD_WORKSPACE_DIRECTORY:-}" ]]; then
  echo "BUILD_WORKSPACE_DIRECTORY is not set. Run this target with 'bazel run //:rust_checks'." >&2
  exit 1
fi

cd "${BUILD_WORKSPACE_DIRECTORY}"

cargo build --workspace
cargo test --workspace
