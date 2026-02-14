#!/usr/bin/env bash
set -euo pipefail

ensure_command() {
  local cmd="$1"
  shift
  if command -v "${cmd}" >/dev/null 2>&1; then
    return 0
  fi
  local candidate
  for candidate in "$@"; do
    if [[ -n "${candidate}" && -d "${candidate}" ]]; then
      PATH="${candidate}:${PATH}"
    fi
  done
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "missing required command: ${cmd}" >&2
    exit 127
  fi
}

ensure_command cargo "${CARGO_HOME:-}/bin" "${HOME:-}/.cargo/bin" "/home/runner/.cargo/bin"

workspace="${1:?workspace path is required}"
tmp_target_dir="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-cargo-build.XXXXXX")"
trap 'rm -rf "${tmp_target_dir}"' EXIT

cd "${workspace}"
CARGO_TARGET_DIR="${tmp_target_dir}" cargo build --workspace
