#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/setup_team_skills.sh [--skills-file <path>]
  scripts/setup_team_skills.sh [--skills-file=<path>]
  scripts/setup_team_skills.sh [--install-dir <path>]
  scripts/setup_team_skills.sh [--install-dir=<path>]
  scripts/setup_team_skills.sh [--use-repo-skill-paths]

Description:
  Add Team coordinator/worker/deliberation skill files into AgentHub skills.json.
  By default, copies Team skill files into ~/.agenthub/worktrees/team-skills so
  resulting paths are inside the default safe_paths allow-list.
  Existing skills are preserved and duplicates are removed.

Options:
  --skills-file <path>       Override skills config file path.
                             Default: ~/.agenthub/skills.json
  --install-dir <path>       Destination directory for copied Team skill files.
                             Default: ~/.agenthub/worktrees/team-skills
  --use-repo-skill-paths     Do not copy; write repository skill paths directly.
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

skills_file="${HOME}/.agenthub/skills.json"
install_dir="${HOME}/.agenthub/worktrees/team-skills"
use_repo_skill_paths=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skills-file=*)
      skills_file="${1#*=}"
      if [[ -z "${skills_file}" ]]; then
        echo "error: --skills-file requires a non-empty value" >&2
        usage >&2
        exit 1
      fi
      shift
      ;;
    --install-dir=*)
      install_dir="${1#*=}"
      if [[ -z "${install_dir}" ]]; then
        echo "error: --install-dir requires a non-empty value" >&2
        usage >&2
        exit 1
      fi
      shift
      ;;
    --skills-file)
      if [[ $# -lt 2 ]]; then
        echo "error: --skills-file requires a value" >&2
        usage >&2
        exit 1
      fi
      skills_file="$2"
      if [[ -z "${skills_file}" ]]; then
        echo "error: --skills-file requires a non-empty value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --install-dir)
      if [[ $# -lt 2 ]]; then
        echo "error: --install-dir requires a value" >&2
        usage >&2
        exit 1
      fi
      install_dir="$2"
      if [[ -z "${install_dir}" ]]; then
        echo "error: --install-dir requires a non-empty value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --use-repo-skill-paths)
      use_repo_skill_paths=1
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

skills_file="$(expand_tilde_path "${skills_file}")"
install_dir="$(expand_tilde_path "${install_dir}")"

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required but not found in PATH" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
coordinator_skill="${repo_root}/skills/team/team-coordinator-orchestrator.SKILL.md"
worker_skill="${repo_root}/skills/team/team-worker-executor.SKILL.md"
deliberation_skill="${repo_root}/skills/team/team-deliberation-rules.SKILL.md"

for skill_path in "${coordinator_skill}" "${worker_skill}" "${deliberation_skill}"; do
  if [[ ! -f "${skill_path}" ]]; then
    echo "error: missing skill file: ${skill_path}" >&2
    exit 1
  fi
done

target_coordinator="${coordinator_skill}"
target_worker="${worker_skill}"
target_deliberation="${deliberation_skill}"
if [[ "${use_repo_skill_paths}" -eq 0 ]]; then
  mkdir -p "${install_dir}"
  target_coordinator="${install_dir}/team-coordinator-orchestrator.SKILL.md"
  target_worker="${install_dir}/team-worker-executor.SKILL.md"
  target_deliberation="${install_dir}/team-deliberation-rules.SKILL.md"
  cp "${coordinator_skill}" "${target_coordinator}"
  cp "${worker_skill}" "${target_worker}"
  cp "${deliberation_skill}" "${target_deliberation}"
fi

mkdir -p "$(dirname "${skills_file}")"
if [[ ! -f "${skills_file}" ]]; then
  printf '{"skills":[]}\n' > "${skills_file}"
fi

tmp_file="$(mktemp "${TMPDIR:-/tmp}/agenthub-skills.XXXXXX")"
cleanup() {
  if [[ -n "${tmp_file:-}" && -f "${tmp_file}" ]]; then
    rm -f "${tmp_file}"
  fi
}
trap cleanup EXIT

jq \
  --arg coordinator "${target_coordinator}" \
  --arg worker "${target_worker}" \
  --arg deliberation "${target_deliberation}" \
  '
    .skills = (
      ((.skills | if type == "array" then . else [] end) + [$coordinator, $worker, $deliberation])
      | reduce .[] as $entry (
          {items: [], seen: {}};
          (
            if ($entry | type) == "string" then
              $entry
            elif ($entry | type) == "object" and (($entry.path? | type) == "string") then
              $entry.path
            else
              null
            end
          ) as $key
          | if $key == null then
              .items += [$entry]
            elif (.seen[$key] // false) then
              .
            else
              .items += [$entry] | .seen[$key] = true
            end
        )
      | .items
    )
  ' -- "${skills_file}" > "${tmp_file}"
mv "${tmp_file}" "${skills_file}"
tmp_file=""

echo "updated skills config: ${skills_file}"
echo "added team skills:"
echo "  - ${target_coordinator}"
echo "  - ${target_worker}"
echo "  - ${target_deliberation}"
if [[ "${use_repo_skill_paths}" -eq 0 ]]; then
  echo "installed skill files under: ${install_dir}"
else
  echo "used repository skill paths (no file copy)"
fi
