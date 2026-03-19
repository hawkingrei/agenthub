import {
  DEFAULT_AGENT_PRESET_ID,
  type AgentPresetId,
} from "../../agent_presets";
import {
  buildLeaderForgeDefaultWorkdir,
  type TeamMemberProfileDraft,
} from "./create_helpers";
import {
  DEFAULT_TEAM_LEADER_PROMPT,
  DEFAULT_TEAM_LEADER_SKILLS,
  DEFAULT_TEAM_WORKER_PROMPT,
  DEFAULT_TEAM_WORKER_SKILLS,
} from "./member_helpers";
import { DEFAULT_WORKTREE_ROOT } from "./state";
import { normalizeWorkdirInput, resolveWorkdirForModalOpen } from "../../worktree_defaults";

export type TeamMemberRole = TeamMemberProfileDraft["role"];

export type TeamMemberRoleOption = {
  value: TeamMemberRole;
  label: string;
  description: string;
  disabled: boolean;
};

export type TeamMemberRoleProfile = {
  profileLabel: string;
  intro: string;
  focus: string;
  skillsHint: string;
  promptHint: string;
};

export type TeamForgeDefaults = {
  draft: TeamMemberProfileDraft;
  agentName: string;
  agentWorkdir: string;
  worktreeMode: "use_existing" | "create_worktree";
  worktreeRepo: string;
  worktreeRef: string;
};

type ResolveTeamForgeDefaultsArgs = {
  teamName: string;
  teamSpec?: unknown;
  role: TeamMemberRole;
  workerCount: number;
  defaultWorktreeRoot: string;
  agentPresetId?: AgentPresetId;
};

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function readRuntimePath(value: unknown, key: "worktree_repo" | "workdir"): string {
  const record = asObjectRecord(value);
  const raw = record?.[key];
  return typeof raw === "string" ? normalizeWorkdirInput(raw) : "";
}

function resolveTeamForgeWorkerWorkdir(teamSpec: unknown): string {
  const specRecord = asObjectRecord(teamSpec);
  const members = Array.isArray(specRecord?.members) ? specRecord.members : [];

  for (const entry of members) {
    const member = asObjectRecord(entry);
    if (!member) {
      continue;
    }
    const role = typeof member.role === "string" ? member.role.trim().toLowerCase() : "";
    const runtimeWorkdir = readRuntimePath(member.runtime, "workdir");
    if (role === "leader" && runtimeWorkdir) {
      return runtimeWorkdir;
    }
  }

  for (const entry of members) {
    const member = asObjectRecord(entry);
    if (!member) {
      continue;
    }
    const runtimeWorkdir = readRuntimePath(member.runtime, "workdir");
    if (runtimeWorkdir) {
      return runtimeWorkdir;
    }
  }

  return "";
}

function resolveTeamForgeWorktreeRepo(teamSpec: unknown): string {
  const specRecord = asObjectRecord(teamSpec);
  const members = Array.isArray(specRecord?.members) ? specRecord.members : [];

  for (const entry of members) {
    const member = asObjectRecord(entry);
    if (!member) {
      continue;
    }
    const runtimeRepo = readRuntimePath(member.runtime, "worktree_repo");
    if (runtimeRepo) {
      return runtimeRepo;
    }
  }

  for (const entry of members) {
    const member = asObjectRecord(entry);
    if (!member) {
      continue;
    }
    const role = typeof member.role === "string" ? member.role.trim().toLowerCase() : "";
    const runtimeWorkdir = readRuntimePath(member.runtime, "workdir");
    if (role === "leader" && runtimeWorkdir) {
      return runtimeWorkdir;
    }
  }

  for (const entry of members) {
    const member = asObjectRecord(entry);
    if (!member) {
      continue;
    }
    const runtimeWorkdir = readRuntimePath(member.runtime, "workdir");
    if (runtimeWorkdir) {
      return runtimeWorkdir;
    }
  }

  return "";
}

export function buildTeamMemberProfileDraft(
  role: TeamMemberRole,
  model: string = DEFAULT_AGENT_PRESET_ID
): TeamMemberProfileDraft {
  return {
    member_id: "",
    role,
    description: "",
    model,
    prompt: role === "leader" ? DEFAULT_TEAM_LEADER_PROMPT : DEFAULT_TEAM_WORKER_PROMPT,
    skills:
      role === "leader"
        ? [...DEFAULT_TEAM_LEADER_SKILLS]
        : [...DEFAULT_TEAM_WORKER_SKILLS],
    custom_skills: "",
  };
}

export function buildTeamAgentNameToken(raw: string): string {
  const normalized = raw
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return normalized || "team";
}

export function resolveInitialTeamMemberRole(hasLeader: boolean): TeamMemberRole {
  return hasLeader ? "worker" : "leader";
}

export function resolveTeamMemberRoleOptions(hasLeader: boolean): TeamMemberRoleOption[] {
  return [
    {
      value: "leader",
      label: "Leader",
      description: hasLeader
        ? "Already assigned for this team."
        : "Own planning, review, and final synthesis.",
      disabled: hasLeader,
    },
    {
      value: "worker",
      label: "Worker",
      description: hasLeader
        ? "Deliver execution, evidence, and implementation."
        : "Unlock after the first leader exists.",
      disabled: !hasLeader,
    },
  ];
}

export function resolveTeamMemberRoleProfile(role: TeamMemberRole): TeamMemberRoleProfile {
  if (role === "leader") {
    return {
      profileLabel: "Leader Profile",
      intro: "Configure the planning identity that owns delegation, review, and final synthesis.",
      focus: "Own planning, review, and final synthesis.",
      skillsHint:
        "Keep orchestration and deliberation skills pinned, then add helpers only when the leader truly needs them.",
      promptHint:
        "Keep the prompt focused on planning policy, delegation rules, and synthesis expectations.",
    };
  }
  return {
    profileLabel: "Worker Profile",
    intro: "Configure the execution identity that implements scoped work and reports evidence.",
    focus: "Deliver implementation, validation, and execution evidence.",
    skillsHint:
      "Keep the execution profile lean, then add optional helpers only for the assigned delivery lane.",
    promptHint:
      "Keep the prompt focused on scope boundaries, evidence quality, and handoff discipline.",
  };
}

export function resolveTeamForgeDefaults({
  teamName,
  teamSpec,
  role,
  workerCount,
  defaultWorktreeRoot,
  agentPresetId = DEFAULT_AGENT_PRESET_ID,
}: ResolveTeamForgeDefaultsArgs): TeamForgeDefaults {
  const prefix = buildTeamAgentNameToken(teamName);
  const agentName =
    role === "leader"
      ? `${prefix}-leader`
      : `${prefix}-worker-${Math.max(1, workerCount + 1)}`;
  const normalizedRoot =
    normalizeWorkdirInput(defaultWorktreeRoot) || DEFAULT_WORKTREE_ROOT;

  return {
    draft: buildTeamMemberProfileDraft(role, agentPresetId),
    agentName,
    agentWorkdir:
      role === "leader"
        ? buildLeaderForgeDefaultWorkdir(normalizedRoot, agentName)
        : resolveWorkdirForModalOpen(
            resolveTeamForgeWorkerWorkdir(teamSpec),
            "use_existing",
            defaultWorktreeRoot,
            DEFAULT_WORKTREE_ROOT
          ),
    worktreeMode: "use_existing",
    worktreeRepo: resolveTeamForgeWorktreeRepo(teamSpec),
    worktreeRef: "",
  };
}
