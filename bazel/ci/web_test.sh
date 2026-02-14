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

ensure_command npm "/opt/hostedtoolcache/node/20.20.0/x64/bin" "/opt/hostedtoolcache/node/20.19.0/x64/bin"

workspace="${1:?workspace path is required}"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-web-test.XXXXXX")"
trap 'rm -rf "${tmp_dir}"' EXIT

cp -R "${workspace}/web" "${tmp_dir}/web"
cd "${tmp_dir}/web"

npm ci
npm run lint
npm run test
