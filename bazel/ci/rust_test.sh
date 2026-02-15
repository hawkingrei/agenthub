#!/usr/bin/env bash
set -euo pipefail

ensure_rustup_env() {
  local user_name="${USER:-}"
  if [[ -z "${user_name}" ]]; then
    user_name="$(id -un 2>/dev/null || true)"
  fi
  local user_home=""
  if [[ -n "${user_name}" ]]; then
    if [[ -d "/Users/${user_name}" ]]; then
      user_home="/Users/${user_name}"
    elif [[ -d "/home/${user_name}" ]]; then
      user_home="/home/${user_name}"
    fi
  fi

  if [[ -z "${CARGO_HOME:-}" ]]; then
    if [[ -d "${HOME:-}/.cargo" ]]; then
      export CARGO_HOME="${HOME}/.cargo"
    elif [[ -n "${user_home}" && -d "${user_home}/.cargo" ]]; then
      export CARGO_HOME="${user_home}/.cargo"
    elif [[ -d "/home/runner/.cargo" ]]; then
      export CARGO_HOME="/home/runner/.cargo"
    fi
  fi
  if [[ -z "${RUSTUP_HOME:-}" ]]; then
    if [[ -d "${HOME:-}/.rustup" ]]; then
      export RUSTUP_HOME="${HOME}/.rustup"
    elif [[ -n "${user_home}" && -d "${user_home}/.rustup" ]]; then
      export RUSTUP_HOME="${user_home}/.rustup"
    elif [[ -d "/home/runner/.rustup" ]]; then
      export RUSTUP_HOME="/home/runner/.rustup"
    fi
  fi
}

ensure_cargo_home_writable() {
  local candidate="${CARGO_HOME:-}"
  if [[ -z "${candidate}" && -n "${HOME:-}" ]]; then
    candidate="${HOME}/.cargo"
  fi

  if [[ -n "${candidate}" ]]; then
    mkdir -p "${candidate}" >/dev/null 2>&1 || true
    if [[ -w "${candidate}" ]]; then
      export CARGO_HOME="${candidate}"
      return 0
    fi
  fi

  local fallback
  fallback="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-cargo-home.XXXXXX")"
  export CARGO_HOME="${fallback}"
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
ensure_cargo_home_writable
user_name="${USER:-$(id -un 2>/dev/null || true)}"
ensure_command \
  cargo \
  "${CARGO_HOME:-}/bin" \
  "${HOME:-}/.cargo/bin" \
  "/Users/${user_name}/.cargo/bin" \
  "/home/${user_name}/.cargo/bin" \
  "/home/runner/.cargo/bin" \
  "/opt/homebrew/bin" \
  "/usr/local/bin"

workspace="${1:?workspace path is required}"
workspace_abs="$(cd "${workspace}" && pwd)"
tmp_target_dir="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-cargo-test.XXXXXX")"
trap 'rm -rf "${tmp_target_dir}"' EXIT

# RustEmbed expects web/dist to exist during compilation.
if [[ ! -f "${workspace_abs}/web/dist/index.html" ]]; then
  bash "${workspace_abs}/bazel/ci/web_build.sh" "${workspace_abs}" "${workspace_abs}/web/dist"
fi

cd "${workspace_abs}"
CARGO_TARGET_DIR="${tmp_target_dir}" cargo test --workspace
