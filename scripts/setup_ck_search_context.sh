#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/setup_ck_search_context.sh [--mcp-file <path>]
  scripts/setup_ck_search_context.sh [--mcp-file=<path>]
  scripts/setup_ck_search_context.sh [--server-name <name>]
  scripts/setup_ck_search_context.sh [--server-name=<name>]
  scripts/setup_ck_search_context.sh [--ck-command <path_or_name>]
  scripts/setup_ck_search_context.sh [--ck-command=<path_or_name>]
  scripts/setup_ck_search_context.sh [--cwd <path>]
  scripts/setup_ck_search_context.sh [--cwd=<path>]
  scripts/setup_ck_search_context.sh [--info-root <path>]
  scripts/setup_ck_search_context.sh [--info-root=<path>]
  scripts/setup_ck_search_context.sh [--skip-info-layout]

Description:
  Bootstrap ck search for AgentHub ACP sessions via ~/.agenthub/mcp.json and
  create a local research layout under .info for papers and clippings.

Options:
  --mcp-file <path>         MCP config file path (default: ~/.agenthub/mcp.json)
  --server-name <name>      mcpServers key name (default: ck-search)
  --ck-command <value>      ck command path or binary name (default: ck)
  --cwd <path>              Working directory used by ck MCP server.
                            Default: repository root (script parent)
  --info-root <path>        Local research root directory.
                            Default: <repo_root>/.info
  --skip-info-layout        Skip creating .info/papers and .info/clippings.
EOF
}

expand_tilde_path() {
  local raw="$1"
  if [[ "$raw" == "~" ]]; then
    printf '%s\n' "${HOME}"
  elif [[ "$raw" == "~/"* ]]; then
    printf '%s\n' "${HOME}${raw:1}"
  else
    printf '%s\n' "$raw"
  fi
}

require_command() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "error: required command not found: $cmd" >&2
    exit 1
  fi
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mcp_file="${HOME}/.agenthub/mcp.json"
server_name="ck-search"
ck_command="ck"
server_cwd="${repo_root}"
info_root="${repo_root}/.info"
skip_info_layout=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mcp-file=*)
      mcp_file="${1#*=}"
      shift
      ;;
    --mcp-file)
      if [[ $# -lt 2 ]]; then
        echo "error: --mcp-file requires a value" >&2
        usage >&2
        exit 1
      fi
      mcp_file="$2"
      shift 2
      ;;
    --server-name=*)
      server_name="${1#*=}"
      shift
      ;;
    --server-name)
      if [[ $# -lt 2 ]]; then
        echo "error: --server-name requires a value" >&2
        usage >&2
        exit 1
      fi
      server_name="$2"
      shift 2
      ;;
    --ck-command=*)
      ck_command="${1#*=}"
      shift
      ;;
    --ck-command)
      if [[ $# -lt 2 ]]; then
        echo "error: --ck-command requires a value" >&2
        usage >&2
        exit 1
      fi
      ck_command="$2"
      shift 2
      ;;
    --cwd=*)
      server_cwd="${1#*=}"
      shift
      ;;
    --cwd)
      if [[ $# -lt 2 ]]; then
        echo "error: --cwd requires a value" >&2
        usage >&2
        exit 1
      fi
      server_cwd="$2"
      shift 2
      ;;
    --info-root=*)
      info_root="${1#*=}"
      shift
      ;;
    --info-root)
      if [[ $# -lt 2 ]]; then
        echo "error: --info-root requires a value" >&2
        usage >&2
        exit 1
      fi
      info_root="$2"
      shift 2
      ;;
    --skip-info-layout)
      skip_info_layout=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument '$1'" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "${mcp_file}" || -z "${server_name}" || -z "${ck_command}" || -z "${server_cwd}" || -z "${info_root}" ]]; then
  echo "error: argument value cannot be empty" >&2
  usage >&2
  exit 1
fi

mcp_file="$(expand_tilde_path "${mcp_file}")"
server_cwd="$(expand_tilde_path "${server_cwd}")"
info_root="$(expand_tilde_path "${info_root}")"

require_command jq
require_command "${ck_command}"

mkdir -p "$(dirname "${mcp_file}")"
if [[ ! -f "${mcp_file}" ]]; then
  printf '{"mcpServers":{}}\n' > "${mcp_file}"
fi

tmp_file="$(mktemp "${TMPDIR:-/tmp}/agenthub-ck-mcp.XXXXXX")"
cleanup() {
  if [[ -n "${tmp_file:-}" && -f "${tmp_file}" ]]; then
    rm -f "${tmp_file}"
  fi
}
trap cleanup EXIT

jq \
  --arg name "${server_name}" \
  --arg command "${ck_command}" \
  --arg cwd "${server_cwd}" \
  '
    .mcpServers = (
      (.mcpServers | if type == "object" then . else {} end)
      + {
          ($name): {
            "command": $command,
            "args": ["--serve"],
            "cwd": $cwd
          }
        }
    )
  ' -- "${mcp_file}" > "${tmp_file}"
mv "${tmp_file}" "${mcp_file}"
tmp_file=""

if [[ "${skip_info_layout}" -eq 0 ]]; then
  mkdir -p \
    "${info_root}/papers" \
    "${info_root}/clippings"
fi

echo "updated MCP config: ${mcp_file}"
echo "configured server: ${server_name} -> ${ck_command} --serve"
echo "configured cwd: ${server_cwd}"
if [[ "${skip_info_layout}" -eq 0 ]]; then
  echo "created research layout:"
  echo "  - ${info_root}/papers"
  echo "  - ${info_root}/clippings"
else
  echo "skipped .info research layout creation"
fi
