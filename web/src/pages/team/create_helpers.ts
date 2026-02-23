import { listAgentPresets } from "../../agent_presets";
import {
  DEFAULT_TEAM_LEADER_PROMPT,
  DEFAULT_TEAM_LEADER_SKILLS,
  DEFAULT_TEAM_WORKER_PROMPT,
  DEFAULT_TEAM_WORKER_SKILLS,
  REQUIRED_TEAM_LEADER_SKILLS,
  REQUIRED_TEAM_WORKER_SKILLS,
  normalizeSkillSelection,
  type WorkerDraft,
} from "./member_helpers";
import { DEFAULT_WORKTREE_ROOT, type CreateTeamStage } from "./state";

const TEAM_MODEL_PRESET_OPTIONS = listAgentPresets().map((preset) => ({
  value: preset.id,
  label: preset.label,
}));
const TEAM_MODEL_PRESET_VALUES = new Set(
  TEAM_MODEL_PRESET_OPTIONS.map((option) => option.value)
);

type TeamStepDraft = {
  step_key: string;
  member_id: string;
  depends_on: string[];
};

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

export function buildTeamSpecFromForm(
  leaderMemberId: string,
  leaderModel: string,
  leaderPrompt: string,
  leaderSkills: string[],
  leaderCustomSkills: string,
  workers: WorkerDraft[]
): unknown {
  const leaderId = leaderMemberId.trim();
  const normalizedWorkers = workers
    .map((worker) => ({
      member_id: worker.member_id.trim(),
      model: worker.model.trim(),
      prompt: worker.prompt.trim() || DEFAULT_TEAM_WORKER_PROMPT,
      skills: normalizeSkillSelection(
        worker.skills,
        worker.custom_skills,
        DEFAULT_TEAM_WORKER_SKILLS,
        REQUIRED_TEAM_WORKER_SKILLS
      ),
    }))
    .filter((worker) => worker.member_id.length > 0);
  const steps = buildDefaultWorkflowSteps(
    leaderId,
    normalizedWorkers.map((worker) => worker.member_id)
  );

  const members = [
    {
      member_id: leaderId,
      role: "leader",
      model: leaderModel.trim() || undefined,
      prompt: leaderPrompt.trim() || DEFAULT_TEAM_LEADER_PROMPT,
      skills: normalizeSkillSelection(
        leaderSkills,
        leaderCustomSkills,
        DEFAULT_TEAM_LEADER_SKILLS,
        REQUIRED_TEAM_LEADER_SKILLS
      ),
    },
    ...normalizedWorkers.map((worker) => ({
      member_id: worker.member_id,
      role: "worker",
      model: worker.model || undefined,
      prompt: worker.prompt,
      skills: worker.skills,
    })),
  ];

  return {
    spec_version: 1,
    entrypoint: steps[0]?.step_key ?? leaderId,
    leader_member_id: leaderId,
    members,
    steps,
  };
}

function buildDefaultWorkflowSteps(
  leaderMemberId: string,
  workerMemberIds: string[]
): TeamStepDraft[] {
  if (!leaderMemberId.trim()) {
    return [];
  }
  const planningStep: TeamStepDraft = {
    step_key: "leader_plan",
    member_id: leaderMemberId,
    depends_on: [],
  };
  if (workerMemberIds.length === 0) {
    return [planningStep];
  }
  const workerSteps = workerMemberIds.map((memberId, index) => ({
    step_key: `worker_${index + 1}_${toStepKeyToken(memberId)}`,
    member_id: memberId,
    depends_on: [planningStep.step_key],
  }));
  const synthesizeStep: TeamStepDraft = {
    step_key: "leader_synthesize",
    member_id: leaderMemberId,
    depends_on: workerSteps.map((step) => step.step_key),
  };
  return [planningStep, ...workerSteps, synthesizeStep];
}

function toStepKeyToken(raw: string): string {
  const normalized = raw
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return normalized || "worker";
}

export function parseErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    const msg = err.message ?? "request failed";
    if (!msg.trim().startsWith("{")) {
      return msg;
    }
    try {
      const parsed = JSON.parse(msg) as { error?: string };
      if (typeof parsed.error === "string" && parsed.error) {
        return parsed.error;
      }
      return msg;
    } catch {
      return msg;
    }
  }
  return String(err);
}

export function buildLeaderForgeDefaultWorkdir(
  defaultRoot: string,
  agentName: string,
  seed: number = Date.now()
): string {
  const normalizedRoot = defaultRoot.trim() || DEFAULT_WORKTREE_ROOT;
  const root = normalizedRoot.replace(/[\\/]+$/, "");
  const normalizedName = agentName
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  const nameToken = normalizedName || "leader";
  const seedToken = Math.max(0, Math.floor(seed)).toString(36);
  return `${root}/${nameToken}-${seedToken}`;
}

export function formatTeamForgeWorktreeError(err: unknown): string | null {
  const msg = parseErrorMessage(err);
  const lower = msg.toLowerCase();
  if (!lower.includes("worktree") && !lower.includes("workdir")) return null;
  if (lower.includes("workdir not allowed")) {
    return "Workdir not allowed. Add the path to Safe Paths before creating this agent.";
  }
  if (lower.includes("worktree repo is required") || lower.includes("worktree_repo required")) {
    return "Worktree repo is required for the selected mode.";
  }
  if (lower.includes("worktree does not exist")) {
    return "Worktree does not exist. Use Create Worktree or choose an existing workdir.";
  }
  if (lower.includes("workdir is not empty")) {
    return "Workdir is not empty. Choose an empty directory for Create Worktree.";
  }
  if (lower.includes("git worktree add failed")) {
    return `Git worktree add failed. ${msg}`;
  }
  return msg;
}

export function parseRequiredJson(raw: string, field: string): unknown {
  const trimmed = raw.trim();
  if (!trimmed) {
    throw new Error(`${field} is required`);
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    throw new Error(`${field} must be valid JSON`);
  }
}

export function parseOptionalJson(raw: string, field: string): unknown | undefined {
  const trimmed = raw.trim();
  if (!trimmed) {
    return undefined;
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch (err) {
    const detail = err instanceof Error ? err.message : "unknown parse error";
    throw new Error(`${field} must be valid JSON (${detail})`);
  }
}

export function parseOptionalInteger(raw: string, field: string): number | undefined {
  const trimmed = raw.trim();
  if (!trimmed) {
    return undefined;
  }
  const parsed = Number.parseInt(trimmed, 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    throw new Error(`${field} must be a non-negative integer`);
  }
  return parsed;
}

export function clampCreateTeamStage(next: number): CreateTeamStage {
  if (next <= 0) return 0;
  if (next >= 3) return 3;
  return next as CreateTeamStage;
}

export function resolveTeamModelOptions(currentModel: string): Array<{
  value: string;
  label: string;
}> {
  const options = [{ value: "", label: "Use default model" }, ...TEAM_MODEL_PRESET_OPTIONS];
  const normalized = currentModel.trim();
  if (normalized && !TEAM_MODEL_PRESET_VALUES.has(normalized)) {
    options.push({ value: normalized, label: `Custom (${normalized})` });
  }
  return options;
}

export function collectTeamSpecMemberIds(spec: unknown): string[] {
  const specObj = asObjectRecord(spec);
  if (!specObj) {
    return [];
  }
  const members = Array.isArray(specObj.members) ? specObj.members : [];
  const seen = new Set<string>();
  const result: string[] = [];
  for (const member of members) {
    const memberObj = asObjectRecord(member);
    if (!memberObj) {
      continue;
    }
    const memberId =
      typeof memberObj.member_id === "string" ? memberObj.member_id.trim() : "";
    if (!memberId || seen.has(memberId)) {
      continue;
    }
    seen.add(memberId);
    result.push(memberId);
  }
  return result;
}

export function resolveUnusedTeamForgeAgentIds(
  teamForgeAgentIds: string[],
  spec: unknown
): string[] {
  const selectedMemberIds = new Set(collectTeamSpecMemberIds(spec));
  const seen = new Set<string>();
  const stale: string[] = [];
  for (const rawId of teamForgeAgentIds) {
    const agentId = rawId.trim();
    if (!agentId || seen.has(agentId)) {
      continue;
    }
    seen.add(agentId);
    if (!selectedMemberIds.has(agentId)) {
      stale.push(agentId);
    }
  }
  return stale;
}
