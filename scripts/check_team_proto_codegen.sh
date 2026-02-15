#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

placeholder_created=0
cleanup() {
  if [[ ${placeholder_created} -eq 1 ]]; then
    rm -f web/dist/index.html
    rmdir web/dist 2>/dev/null || true
  fi
}
trap cleanup EXIT

if [[ ! -f web/dist/index.html ]]; then
  mkdir -p web/dist
  printf '<!doctype html><title>placeholder</title>\n' > web/dist/index.html
  placeholder_created=1
fi

if git -c core.fsmonitor=false ls-files | grep -qE 'agenthub\.internal\.v1\.rs$'; then
  echo "checked-in generated protobuf rust files are not allowed"
  echo "please remove tracked generated files and rely on build-time codegen"
  exit 1
fi

echo "running cargo check to trigger protobuf code generation"
cargo check --locked --quiet

generated_files="$(find target -type f -path '*/build/*/out/agenthub.internal.v1.rs')"
if [[ -z "${generated_files}" ]]; then
  echo "failed to locate generated protobuf rust output"
  echo "expected pattern: target/*/build/*/out/agenthub.internal.v1.rs"
  exit 1
fi

latest_generated="$(printf '%s\n' "${generated_files}" | xargs ls -t | head -n1)"

if ! grep -q 'pub trait TeamInternalControl' "${latest_generated}"; then
  echo "generated file missing TeamInternalControl trait"
  echo "file: ${latest_generated}"
  exit 1
fi

if ! grep -q 'pub struct SendActorMessageRequest' "${latest_generated}"; then
  echo "generated file missing SendActorMessageRequest message"
  echo "file: ${latest_generated}"
  exit 1
fi

echo "protobuf codegen check passed: ${latest_generated}"
