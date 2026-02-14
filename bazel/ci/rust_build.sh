#!/usr/bin/env bash
set -euo pipefail

ensure_rustup_env() {
  if [[ -z "${CARGO_HOME:-}" ]]; then
    if [[ -d "${HOME:-}/.cargo" ]]; then
      export CARGO_HOME="${HOME}/.cargo"
    elif [[ -d "/home/runner/.cargo" ]]; then
      export CARGO_HOME="/home/runner/.cargo"
    fi
  fi
  if [[ -z "${RUSTUP_HOME:-}" ]]; then
    if [[ -d "${HOME:-}/.rustup" ]]; then
      export RUSTUP_HOME="${HOME}/.rustup"
    elif [[ -d "/home/runner/.rustup" ]]; then
      export RUSTUP_HOME="/home/runner/.rustup"
    fi
  fi
}

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

ensure_rustup_env
ensure_command cargo "${CARGO_HOME:-}/bin" "${HOME:-}/.cargo/bin" "/home/runner/.cargo/bin"

workspace="${1:?workspace path is required}"
workspace_abs="$(cd "${workspace}" && pwd)"
tmp_target_dir="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-cargo-build.XXXXXX")"
trap 'rm -rf "${tmp_target_dir}"' EXIT

# RustEmbed expects web/dist to exist during compilation.
if [[ ! -f "${workspace_abs}/web/dist/index.html" ]]; then
  bash "${workspace_abs}/bazel/ci/web_build.sh" "${workspace_abs}" "${workspace_abs}/web/dist"
fi

cd "${workspace_abs}"
CARGO_TARGET_DIR="${tmp_target_dir}" cargo build --workspace
