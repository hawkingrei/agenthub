#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/setup_team_skills.sh [--skills-file <path>]

Description:
  Add Team leader/worker skill files from this repository into AgentHub
  skills.json. Existing skills are preserved and duplicates are removed.

Options:
  --skills-file <path>  Override skills config file path.
                        Default: ~/.agenthub/skills.json
EOF
}

skills_file="${HOME}/.agenthub/skills.json"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skills-file)
      if [[ $# -lt 2 ]]; then
        echo "error: --skills-file requires a value" >&2
        usage >&2
        exit 1
      fi
      skills_file="$2"
      shift 2
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

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required but not found in PATH" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
leader_skill="${repo_root}/skills/team/team-leader-orchestrator.SKILL.md"
worker_skill="${repo_root}/skills/team/team-worker-executor.SKILL.md"

for skill_path in "${leader_skill}" "${worker_skill}"; do
  if [[ ! -f "${skill_path}" ]]; then
    echo "error: missing skill file: ${skill_path}" >&2
    exit 1
  fi
done

mkdir -p "$(dirname "${skills_file}")"
if [[ ! -f "${skills_file}" ]]; then
  printf '{"skills":[]}\n' > "${skills_file}"
fi

tmp_file="$(mktemp)"
jq \
  --arg leader "${leader_skill}" \
  --arg worker "${worker_skill}" \
  '
    .skills = (((.skills // []) + [$leader, $worker])
      | map(select(type == "string"))
      | unique)
  ' "${skills_file}" > "${tmp_file}"
mv "${tmp_file}" "${skills_file}"

echo "updated skills config: ${skills_file}"
echo "added team skills:"
echo "  - ${leader_skill}"
echo "  - ${worker_skill}"
