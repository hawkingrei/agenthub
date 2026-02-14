#!/usr/bin/env bash
set -euo pipefail

workspace="${1:?workspace path is required}"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/agenthub-web-build.XXXXXX")"
trap 'rm -rf "${tmp_dir}"' EXIT

cp -R "${workspace}/web" "${tmp_dir}/web"
cd "${tmp_dir}/web"

npm ci
npm run build
