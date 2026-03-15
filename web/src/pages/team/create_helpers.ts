import type { AgentRecord } from "../../api";
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

const DEFAULT_TEAM_PLAN_STEP_KEY = "leader_plan";

type TeamStepDraft = {
  step_key: string;
  member_id: string;
  depends_on: string[];
};

export type TeamMemberProfileDraft = {
  member_id: string;
  role: "leader" | "worker";
  description: string;
  model: string;
  prompt: string;
  skills: string[];
  custom_skills: string;
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
  workers: WorkerDraft[],
  teamForgeAgents: AgentRecord[]
): unknown {
  const leaderId = leaderMemberId.trim();
  const forgeAgentById = new Map(teamForgeAgents.map((agent) => [agent.id, agent]));
  const normalizedWorkers = workers
    .map((worker) => ({
      member_id: worker.member_id.trim(),
      description: worker.description.trim(),
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
      runtime: buildMemberRuntimeHint(forgeAgentById.get(leaderId)),
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
      description: worker.description || undefined,
      model: worker.model || undefined,
      prompt: worker.prompt,
      runtime: buildMemberRuntimeHint(forgeAgentById.get(worker.member_id)),
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

function cloneSpecObject(spec: unknown): Record<string, unknown> {
  const specObj = asObjectRecord(spec);
  if (!specObj) {
    return {};
  }
  return JSON.parse(JSON.stringify(specObj)) as Record<string, unknown>;
}

function readMemberId(member: Record<string, unknown>): string {
  return typeof member.member_id === "string" ? member.member_id.trim() : "";
}

function readMemberRole(member: Record<string, unknown>): string {
  return typeof member.role === "string" ? member.role.trim() : "";
}

export function buildEmptyTeamSpec(): unknown {
  return {
    spec_version: 1,
    members: [],
  };
}

export function teamSpecHasConfiguredMembers(spec: unknown): boolean {
  return collectTeamSpecMemberIds(spec).length > 0;
}

export function teamSpecHasLeader(spec: unknown): boolean {
  const specObj = asObjectRecord(spec);
  if (!specObj) {
    return false;
  }
  const members = Array.isArray(specObj.members) ? specObj.members : [];
  return members.some((member) => {
    const memberObj = asObjectRecord(member);
    if (!memberObj) {
      return false;
    }
    return readMemberRole(memberObj) === "leader";
  });
}

export function appendTeamMemberToSpec(
  spec: unknown,
  draft: TeamMemberProfileDraft,
  agent: AgentRecord
): unknown {
  const memberId = draft.member_id.trim();
  if (!memberId) {
    throw new Error("Member id is required");
  }
  const role = draft.role === "leader" ? "leader" : "worker";
  const nextSpec = cloneSpecObject(spec);
  const existingMembers = Array.isArray(nextSpec.members)
    ? nextSpec.members
        .map((member) => asObjectRecord(member))
        .filter((member): member is Record<string, unknown> => Boolean(member))
    : [];
  if (existingMembers.some((member) => readMemberId(member) === memberId)) {
    throw new Error(`Team already includes member ${memberId}`);
  }
  const leaderId = existingMembers.find((member) => readMemberRole(member) === "leader");
  if (role === "leader" && leaderId) {
    throw new Error("Team already has a leader");
  }
  if (role === "worker" && !leaderId) {
    throw new Error("Create the first agent before adding more agents");
  }

  const normalizedSkills =
    role === "leader"
      ? normalizeSkillSelection(
          draft.skills,
          draft.custom_skills,
          DEFAULT_TEAM_LEADER_SKILLS,
          REQUIRED_TEAM_LEADER_SKILLS
        )
      : normalizeSkillSelection(
          draft.skills,
          draft.custom_skills,
          DEFAULT_TEAM_WORKER_SKILLS,
          REQUIRED_TEAM_WORKER_SKILLS
        );
  const prompt =
    draft.prompt.trim() ||
    (role === "leader" ? DEFAULT_TEAM_LEADER_PROMPT : DEFAULT_TEAM_WORKER_PROMPT);

  existingMembers.push({
    member_id: memberId,
    role,
    description: draft.description.trim() || undefined,
    model: draft.model.trim() || undefined,
    prompt,
    runtime: buildMemberRuntimeHint(agent),
    skills: normalizedSkills,
  });

  const resolvedLeaderId =
    role === "leader"
      ? memberId
      : existingMembers.find((member) => readMemberRole(member) === "leader")?.member_id;
  if (typeof resolvedLeaderId !== "string" || !resolvedLeaderId.trim()) {
    throw new Error("Team leader is required");
  }
  const normalizedLeaderId = resolvedLeaderId.trim();
  const workerMemberIds = existingMembers
    .map((member) => readMemberId(member))
    .filter((candidate) => candidate.length > 0 && candidate !== normalizedLeaderId);

  nextSpec.spec_version = 1;
  nextSpec.members = existingMembers;
  nextSpec.leader_member_id = normalizedLeaderId;
  nextSpec.steps = buildDefaultWorkflowSteps(normalizedLeaderId, workerMemberIds);
  nextSpec.entrypoint = DEFAULT_TEAM_PLAN_STEP_KEY;
  return nextSpec;
}

function buildMemberRuntimeHint(agent: AgentRecord | undefined): Record<string, unknown> | undefined {
  if (!agent) {
    return undefined;
  }
  return {
    name: agent.name,
    workdir: agent.workdir,
    worktree_mode: agent.worktree_mode,
    worktree_repo: agent.worktree_repo ?? null,
    worktree_ref: agent.worktree_ref ?? null,
    code_mode: agent.code_mode,
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

export type TeamForgeCleanupResult = {
  deletedForgeAgentIds: string[];
  cleanupErrors: string[];
};

type DeleteAgentFn = (token: string, agentId: string) => Promise<void>;

export async function cleanupUnusedTeamForgeAgents(
  token: string,
  staleForgeAgentIds: string[],
  deleteAgent: DeleteAgentFn
): Promise<TeamForgeCleanupResult> {
  const normalizedIds = Array.from(
    new Set(
      staleForgeAgentIds
        .map((rawId) => rawId.trim())
        .filter((agentId) => agentId.length > 0)
    )
  );
  if (normalizedIds.length === 0) {
    return { deletedForgeAgentIds: [], cleanupErrors: [] };
  }
  const cleanupResults = await Promise.all(
    normalizedIds.map(async (agentId) => {
      try {
        await deleteAgent(token, agentId);
        return { agentId, error: null as string | null };
      } catch (err) {
        return { agentId, error: `${agentId}: ${parseErrorMessage(err)}` };
      }
    })
  );
  const deletedForgeAgentIds: string[] = [];
  const cleanupErrors: string[] = [];
  for (const result of cleanupResults) {
    if (result.error) {
      cleanupErrors.push(result.error);
      continue;
    }
    deletedForgeAgentIds.push(result.agentId);
  }
  return { deletedForgeAgentIds, cleanupErrors };
}

export function buildTeamForgeCleanupWarning(cleanupErrors: string[]): string | null {
  if (cleanupErrors.length === 0) {
    return null;
  }
  return `Team created, but failed to clean up ${cleanupErrors.length} unused forged agent(s): ${cleanupErrors.join("; ")}`;
}
