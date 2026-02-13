#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${BUILD_WORKSPACE_DIRECTORY:-}" ]]; then
  echo "BUILD_WORKSPACE_DIRECTORY is not set. Run this target with 'bazel run //:ci_checks'." >&2
  exit 1
fi

"${BUILD_WORKSPACE_DIRECTORY}/bazel/ci/rust_checks.sh"
"${BUILD_WORKSPACE_DIRECTORY}/bazel/ci/web_checks.sh"
