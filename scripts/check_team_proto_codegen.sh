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

generated_files=()
while IFS= read -r -d '' generated_file; do
  generated_files+=("${generated_file}")
done < <(find target -type f -path '*/build/*/out/agenthub.internal.v1.rs' -print0)

if [[ ${#generated_files[@]} -eq 0 ]]; then
  echo "failed to locate generated protobuf rust output"
  echo "expected pattern: target/*/build/*/out/agenthub.internal.v1.rs"
  exit 1
fi

file_mtime() {
  local file_path="$1"
  if stat -c '%Y' "${file_path}" >/dev/null 2>&1; then
    stat -c '%Y' "${file_path}"
  else
    stat -f '%m' "${file_path}"
  fi
}

latest_generated="${generated_files[0]}"
latest_mtime="$(file_mtime "${latest_generated}")"
for generated_file in "${generated_files[@]:1}"; do
  generated_mtime="$(file_mtime "${generated_file}")"
  if (( generated_mtime > latest_mtime )); then
    latest_generated="${generated_file}"
    latest_mtime="${generated_mtime}"
  fi
done

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
