#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  build/codex/fetch-code-mode-host.sh TARGET OUTPUT

Download the official Codex Code Mode Host that matches AgentHub's pinned
Codex dependency and verify its release checksum before installing it at OUTPUT.
USAGE
}

if [[ $# -ne 2 ]]; then
  usage >&2
  exit 2
fi

target="$1"
output="$2"

# Keep these values aligned with the official Codex pin in MODULE.bazel and Cargo.toml.
readonly CODEX_REV="90854393966b21e9ebfd21b122334eb09a20c93d"
readonly CODEX_VERSION="0.150.1"

case "${target}" in
  aarch64-apple-darwin)
    artifact_target="aarch64-apple-darwin"
    archive_sha256="0e342801dac30a050bb1930be2ede7940acf139d32092b43be7876556b27c1bf"
    ;;
  aarch64-unknown-linux-gnu)
    # OpenAI publishes the standalone Linux host as a portable musl binary.
    artifact_target="aarch64-unknown-linux-musl"
    archive_sha256="cc934a8aa36dea77ad3096e025cbe7f2097f0083df902e7f3ed77dbf91fa6f9c"
    ;;
  x86_64-unknown-linux-gnu)
    # OpenAI publishes the standalone Linux host as a portable musl binary.
    artifact_target="x86_64-unknown-linux-musl"
    archive_sha256="b47667846125cdf6dbc460c6fdc418afb2ef3926c54f4d999bbfbeb08dee4fc5"
    ;;
  *)
    echo "unsupported AgentHub release target: ${target}" >&2
    exit 2
    ;;
esac

archive_name="codex-code-mode-host-${artifact_target}.tar.gz"
archive_url="https://github.com/openai/codex/releases/download/rust-v${CODEX_VERSION}/${archive_name}"
work_dir="$(mktemp -d)"
trap 'rm -rf "${work_dir}"' EXIT
archive_path="${work_dir}/${archive_name}"
extract_dir="${work_dir}/extract"

curl --fail --location --retry 3 --silent --show-error "${archive_url}" --output "${archive_path}"

if command -v sha256sum >/dev/null 2>&1; then
  actual_sha256="$(sha256sum "${archive_path}" | awk '{print $1}')"
else
  actual_sha256="$(shasum -a 256 "${archive_path}" | awk '{print $1}')"
fi
if [[ "${actual_sha256}" != "${archive_sha256}" ]]; then
  echo "checksum mismatch for ${archive_name}" >&2
  echo "expected: ${archive_sha256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
fi

mkdir -p "${extract_dir}"
tar -xzf "${archive_path}" -C "${extract_dir}"
host_path="${extract_dir}/codex-code-mode-host-${artifact_target}"
if [[ ! -f "${host_path}" ]]; then
  echo "archive does not contain the expected host executable: ${archive_name}" >&2
  exit 1
fi

mkdir -p "$(dirname "${output}")"
install -m 0755 "${host_path}" "${output}"
echo "Installed Codex ${CODEX_VERSION} (${CODEX_REV:0:12}) Code Mode Host for ${target} at ${output}"
