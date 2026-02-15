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

ensure_command \
  npm \
  "/opt/hostedtoolcache/node/20.20.0/x64/bin" \
  "/opt/hostedtoolcache/node/20.19.0/x64/bin" \
  "/opt/homebrew/bin" \
  "/opt/homebrew/opt/node/bin" \
  "/usr/local/bin"

workspace="${1:?workspace path is required}"
dist_output="${2:-}"
if [[ -n "${dist_output}" && "${dist_output}" != /* ]]; then
  dist_output="$(cd "${workspace}" && pwd)/${dist_output#./}"
fi
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-web-build.XXXXXX")"
trap 'rm -rf "${tmp_dir}"' EXIT

# Bazel exposes workspace files as symlinks in sandbox/execroot.
# Use -L to materialize real files so Vite module resolution stays inside tmp_dir.
cp -RL "${workspace}/web" "${tmp_dir}/web"
cd "${tmp_dir}/web"

npm ci
npm run build

if [[ -n "${dist_output}" ]]; then
  rm -rf "${dist_output}"
  mkdir -p "$(dirname "${dist_output}")"
  cp -R "${tmp_dir}/web/dist" "${dist_output}"
fi
