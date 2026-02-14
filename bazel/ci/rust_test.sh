#!/usr/bin/env bash
set -euo pipefail

workspace="${1:?workspace path is required}"
tmp_target_dir="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-cargo-test.XXXXXX")"
trap 'rm -rf "${tmp_target_dir}"' EXIT

cd "${workspace}"
CARGO_TARGET_DIR="${tmp_target_dir}" cargo test --workspace
