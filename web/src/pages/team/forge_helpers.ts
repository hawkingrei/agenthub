import {
  DEFAULT_AGENT_PRESET_ID,
  type AgentPresetId,
} from "../../agent_presets";
import {
  buildLeaderForgeDefaultWorkdir,
  type TeamMemberProfileDraft,
} from "./create_helpers";
import type { TeamPromptDefaultsRecord } from "../../api";
import { EMPTY_TEAM_PROMPT_DEFAULTS, resolveTeamPromptForRole } from "./member_helpers";
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
  promptDefaults?: TeamPromptDefaultsRecord;
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
  model: string = DEFAULT_AGENT_PRESET_ID,
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): TeamMemberProfileDraft {
  return {
    member_id: "",
    role,
    description: "",
    model,
    prompt: resolveTeamPromptForRole(promptDefaults, role),
    skills: [],
    custom_skills: "",
    agent_loop_enabled: false,
    agent_loop_idle_seconds: "",
    agent_loop_prompt: "",
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
      intro: "Add the planning agent that owns delegation, review, and final synthesis.",
      focus: "Own planning, review, and final synthesis.",
      skillsHint:
        "Role skills and system instructions are injected automatically.",
      promptHint:
        "Describe what this leader should own for the team.",
    };
  }
  return {
    profileLabel: "Worker Profile",
    intro: "Add the execution agent that implements scoped work and reports evidence.",
    focus: "Deliver implementation, validation, and execution evidence.",
    skillsHint:
      "Role skills and system instructions are injected automatically.",
    promptHint:
      "Describe what this worker should help with for the team.",
  };
}

export function resolveTeamForgeDefaults({
  teamName,
  teamSpec,
  role,
  workerCount,
  defaultWorktreeRoot,
  agentPresetId = DEFAULT_AGENT_PRESET_ID,
  promptDefaults = EMPTY_TEAM_PROMPT_DEFAULTS,
}: ResolveTeamForgeDefaultsArgs): TeamForgeDefaults {
  const prefix = buildTeamAgentNameToken(teamName);
  const agentName =
    role === "leader"
      ? `${prefix}-leader`
      : `${prefix}-worker-${Math.max(1, workerCount + 1)}`;
  const normalizedRoot =
    normalizeWorkdirInput(defaultWorktreeRoot) || DEFAULT_WORKTREE_ROOT;

  return {
    draft: buildTeamMemberProfileDraft(role, agentPresetId, promptDefaults),
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
