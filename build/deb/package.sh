#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  build/deb/package.sh --version VERSION --arch amd64|arm64 --target TARGET --output-dir DIST

The target release directory must contain both agenthub and agenthubd.
USAGE
}

version=""
arch=""
target=""
output_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      version="${2:-}"
      shift 2
      ;;
    --arch)
      arch="${2:-}"
      shift 2
      ;;
    --target)
      target="${2:-}"
      shift 2
      ;;
    --output-dir)
      output_dir="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "${version}" || -z "${arch}" || -z "${target}" || -z "${output_dir}" ]]; then
  usage >&2
  exit 2
fi

case "${arch}" in
  amd64|arm64) ;;
  *)
    echo "unsupported Debian architecture: ${arch}" >&2
    exit 2
    ;;
esac

binary_dir="target/${target}/release"
for binary in agenthub agenthubd; do
  if [[ ! -x "${binary_dir}/${binary}" ]]; then
    echo "missing executable ${binary_dir}/${binary}" >&2
    exit 1
  fi
done

if [[ "${version}" =~ ^v ]]; then
  version="${version#v}"
fi
version="$(printf '%s' "${version}" | tr '/_ ' '---' | tr -cd '[:alnum:].+~-')"
if [[ -z "${version}" ]]; then
  echo "Debian package version normalized to an empty value" >&2
  exit 2
fi
if [[ ! "${version}" =~ ^[0-9] ]]; then
  version="0.0.0+${version//-/.}"
fi

package_name="agenthub_${version}_${arch}"
work_dir="$(mktemp -d)"
trap 'rm -rf "${work_dir}"' EXIT
package_root="${work_dir}/${package_name}"

install -d -m 0755 "${package_root}/DEBIAN"
install -d -m 0755 "${package_root}/usr/bin"
install -d -m 0755 "${package_root}/usr/lib/systemd/system"
install -d -m 0755 "${package_root}/usr/share/doc/agenthub"

install -m 0755 "${binary_dir}/agenthub" "${package_root}/usr/bin/agenthub"
install -m 0755 "${binary_dir}/agenthubd" "${package_root}/usr/bin/agenthubd"
install -m 0644 build/deb/agenthub.service "${package_root}/usr/lib/systemd/system/agenthub.service"
install -m 0755 build/deb/DEBIAN/postinst "${package_root}/DEBIAN/postinst"
install -m 0755 build/deb/DEBIAN/prerm "${package_root}/DEBIAN/prerm"
install -m 0755 build/deb/DEBIAN/postrm "${package_root}/DEBIAN/postrm"
install -m 0644 README.md "${package_root}/usr/share/doc/agenthub/README.md"
if [[ -f LICENSE ]]; then
  install -m 0644 LICENSE "${package_root}/usr/share/doc/agenthub/copyright"
fi

installed_size="$(du -sk "${package_root}" | awk '{print $1}')"
cat > "${package_root}/DEBIAN/control" <<CONTROL
Package: agenthub
Version: ${version}
Section: admin
Priority: optional
Architecture: ${arch}
Maintainer: AgentHub Maintainers <maintainers@linkerdog.com>
Depends: libc6, libgcc-s1, libsqlite3-0, zlib1g, adduser
Installed-Size: ${installed_size}
Homepage: https://github.com/hawkingrei/agenthub
Description: Single-binary control plane for long-lived AI agents
 AgentHub manages long-lived AI agent sessions, workspaces, output replay, and ACP-backed agent runtimes.
CONTROL

mkdir -p "${output_dir}"
dpkg-deb --build --root-owner-group "${package_root}" "${output_dir}/${package_name}.deb"
