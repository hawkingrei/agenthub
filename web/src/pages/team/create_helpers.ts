import type { AgentRecord, TeamPromptDefaultsRecord } from "../../api";
import {
  DEFAULT_TEAM_COORDINATOR_SKILLS,
  DEFAULT_TEAM_WORKER_SKILLS,
  EMPTY_TEAM_PROMPT_DEFAULTS,
  resolveTeamPromptForRole,
  type WorkerDraft,
} from "./member_helpers";
import { DEFAULT_WORKTREE_ROOT, type CreateTeamStage } from "./state";

const DEFAULT_TEAM_PLAN_STEP_KEY = "coordinator_plan";
const MIN_AGENT_LOOP_IDLE_SECONDS = 10;
const MAX_AGENT_LOOP_IDLE_SECONDS = 86_400;

type TeamStepDraft = {
  step_key: string;
  member_id: string;
  depends_on: string[];
};

export type TeamMemberProfileDraft = {
  member_id: string;
  role: "coordinator" | "worker";
  description: string;
  model: string;
  prompt: string;
  skills: string[];
  custom_skills: string;
  agent_loop_enabled: boolean;
  agent_loop_idle_seconds: string;
  agent_loop_prompt: string;
};

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

export function buildTeamSpecFromForm(
  coordinatorMemberId: string,
  coordinatorModel: string,
  coordinatorPrompt: string,
  workers: WorkerDraft[],
  teamForgeAgents: AgentRecord[],
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): unknown {
  const coordinatorId = coordinatorMemberId.trim();
  const forgeAgentById = new Map(teamForgeAgents.map((agent) => [agent.id, agent]));
  const normalizedWorkers = workers
    .map((worker) => ({
      member_id: worker.member_id.trim(),
      description: worker.description.trim(),
      model: worker.model.trim(),
      prompt: worker.prompt.trim() || promptDefaults.worker_prompt,
    }))
    .filter((worker) => worker.member_id.length > 0);
  const steps = buildDefaultWorkflowSteps(
    coordinatorId,
    normalizedWorkers.map((worker) => worker.member_id)
  );

  const members = [
    {
      member_id: coordinatorId,
      role: "coordinator",
      model: coordinatorModel.trim() || undefined,
      prompt: coordinatorPrompt.trim() || promptDefaults.coordinator_prompt,
      runtime: buildMemberRuntimeHint(forgeAgentById.get(coordinatorId)),
    },
    ...normalizedWorkers.map((worker) => ({
      member_id: worker.member_id,
      role: "worker",
      description: worker.description || undefined,
      model: worker.model || undefined,
      prompt: worker.prompt,
      runtime: buildMemberRuntimeHint(forgeAgentById.get(worker.member_id)),
    })),
  ];

  return {
    spec_version: 1,
    entrypoint: steps[0]?.step_key ?? coordinatorId,
    coordinator_member_id: coordinatorId,
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

export function teamSpecHasCoordinator(spec: unknown): boolean {
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
    return readMemberRole(memberObj) === "coordinator";
  });
}

export function appendTeamMemberToSpec(
  spec: unknown,
  draft: TeamMemberProfileDraft,
  agent: AgentRecord,
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): unknown {
  const memberId = draft.member_id.trim();
  if (!memberId) {
    throw new Error("Member id is required");
  }
  const role = draft.role === "coordinator" ? "coordinator" : "worker";
  const nextSpec = cloneSpecObject(spec);
  const existingMembers = Array.isArray(nextSpec.members)
    ? nextSpec.members
        .map((member) => asObjectRecord(member))
        .filter((member): member is Record<string, unknown> => Boolean(member))
    : [];
  if (existingMembers.some((member) => readMemberId(member) === memberId)) {
    throw new Error(`Team already includes member ${memberId}`);
  }
  const coordinatorMember = existingMembers.find(
    (member) => readMemberRole(member) === "coordinator"
  );
  if (role === "coordinator" && coordinatorMember) {
    throw new Error("Team already has a coordinator");
  }
  if (role === "worker" && !coordinatorMember) {
    throw new Error("Create the first agent before adding more agents");
  }

  const prompt =
    draft.prompt.trim() || resolveTeamPromptForRole(promptDefaults, role);

  existingMembers.push({
    member_id: memberId,
    role,
    description: draft.description.trim() || undefined,
    model: draft.model.trim() || undefined,
    prompt,
    runtime: buildMemberRuntimeHint(agent),
  });

  const resolvedCoordinatorId =
    role === "coordinator"
      ? memberId
      : existingMembers.find((member) => readMemberRole(member) === "coordinator")?.member_id;
  if (typeof resolvedCoordinatorId !== "string" || !resolvedCoordinatorId.trim()) {
    throw new Error("Team coordinator is required");
  }
  const normalizedCoordinatorId = resolvedCoordinatorId.trim();
  const workerMemberIds = existingMembers
    .map((member) => readMemberId(member))
    .filter((candidate) => candidate.length > 0 && candidate !== normalizedCoordinatorId);

  nextSpec.spec_version = 1;
  nextSpec.members = existingMembers;
  nextSpec.coordinator_member_id = normalizedCoordinatorId;
  nextSpec.steps = buildDefaultWorkflowSteps(normalizedCoordinatorId, workerMemberIds);
  nextSpec.entrypoint = DEFAULT_TEAM_PLAN_STEP_KEY;
  return nextSpec;
}

function readOptionalStringField(
  record: Record<string, unknown>,
  field: "description" | "model" | "prompt"
): string {
  const value = record[field];
  return typeof value === "string" ? value.trim() : "";
}

function readRuntimeRecord(member: Record<string, unknown>): Record<string, unknown> | null {
  return asObjectRecord(member.runtime);
}

function readRuntimeLoopEnabled(member: Record<string, unknown>): boolean {
  return readRuntimeRecord(member)?.agent_loop_enabled === true;
}

function normalizeAgentLoopIdleSeconds(value: unknown): string {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return "";
  }
  const normalized = Math.trunc(value);
  if (
    normalized < MIN_AGENT_LOOP_IDLE_SECONDS ||
    normalized > MAX_AGENT_LOOP_IDLE_SECONDS
  ) {
    return "";
  }
  return String(normalized);
}

function readRuntimeLoopIdleSeconds(member: Record<string, unknown>): string {
  return normalizeAgentLoopIdleSeconds(readRuntimeRecord(member)?.agent_loop_idle_seconds);
}

function readRuntimeLoopPrompt(member: Record<string, unknown>): string {
  const value = readRuntimeRecord(member)?.agent_loop_prompt;
  return typeof value === "string" ? value.trim() : "";
}

export function buildTeamMemberDraftFromSpec(
  spec: unknown,
  memberId: string,
  agent?: AgentRecord | null,
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): TeamMemberProfileDraft | null {
  const normalizedMemberId = memberId.trim();
  if (!normalizedMemberId) {
    return null;
  }
  const specObj = asObjectRecord(spec);
  if (!specObj) {
    return null;
  }
  const members = Array.isArray(specObj.members) ? specObj.members : [];
  const member = members
    .map((item) => asObjectRecord(item))
    .find(
      (item): item is Record<string, unknown> =>
        item !== null && readMemberId(item) === normalizedMemberId
    );
  if (!member) {
    return null;
  }
  const role = readMemberRole(member) === "coordinator" ? "coordinator" : "worker";
  return {
    member_id: normalizedMemberId,
    role,
    description: readOptionalStringField(member, "description"),
    model: readOptionalStringField(member, "model"),
    prompt:
      readOptionalStringField(member, "prompt") || resolveTeamPromptForRole(promptDefaults, role),
    skills:
      role === "coordinator"
        ? [...DEFAULT_TEAM_COORDINATOR_SKILLS]
        : [...DEFAULT_TEAM_WORKER_SKILLS],
    custom_skills: "",
    agent_loop_enabled: agent?.agent_loop_enabled ?? readRuntimeLoopEnabled(member),
    agent_loop_idle_seconds:
      agent?.agent_loop_idle_seconds != null
        ? normalizeAgentLoopIdleSeconds(agent.agent_loop_idle_seconds)
        : readRuntimeLoopIdleSeconds(member),
    agent_loop_prompt: agent?.agent_loop_prompt?.trim() || readRuntimeLoopPrompt(member),
  };
}

export function updateTeamMemberProfileInSpec(
  spec: unknown,
  draft: TeamMemberProfileDraft,
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): unknown {
  const memberId = draft.member_id.trim();
  if (!memberId) {
    throw new Error("Member id is required");
  }
  const nextSpec = cloneSpecObject(spec);
  const existingMembers = Array.isArray(nextSpec.members)
    ? nextSpec.members
        .map((member) => asObjectRecord(member))
        .filter((member): member is Record<string, unknown> => Boolean(member))
    : [];
  const memberIndex = existingMembers.findIndex((member) => readMemberId(member) === memberId);
  if (memberIndex < 0) {
    throw new Error(`Team does not include member ${memberId}`);
  }
  const existing = existingMembers[memberIndex];
  const role = readMemberRole(existing) === "coordinator" ? "coordinator" : "worker";
  const prompt =
    draft.prompt.trim() || resolveTeamPromptForRole(promptDefaults, role);
  const loopIdleRaw = draft.agent_loop_idle_seconds.trim();
  const parsedLoopIdleSeconds =
    loopIdleRaw !== "" && /^\d+$/.test(loopIdleRaw)
      ? Number.parseInt(loopIdleRaw, 10)
      : Number.NaN;
  const normalizedLoopIdleSeconds =
    Number.isFinite(parsedLoopIdleSeconds) &&
    parsedLoopIdleSeconds >= MIN_AGENT_LOOP_IDLE_SECONDS &&
    parsedLoopIdleSeconds <= MAX_AGENT_LOOP_IDLE_SECONDS
      ? parsedLoopIdleSeconds
      : undefined;
  existingMembers[memberIndex] = {
    ...existing,
    member_id: memberId,
    role,
    description: draft.description.trim() || undefined,
    model: draft.model.trim() || undefined,
    prompt,
    runtime: {
      ...asObjectRecord(existing.runtime),
      agent_loop_enabled: draft.agent_loop_enabled || undefined,
      agent_loop_idle_seconds: normalizedLoopIdleSeconds,
      agent_loop_prompt: draft.agent_loop_prompt.trim() || undefined,
    },
  };
  nextSpec.members = existingMembers;
  return nextSpec;
}

function buildMemberRuntimeHint(agent: AgentRecord | undefined): Record<string, unknown> | undefined {
  if (!agent) {
    return undefined;
  }
  return {
    name: agent.name,
    target_node_id: agent.target_node_id ?? null,
    workdir: agent.workdir,
    worktree_mode: agent.worktree_mode,
    worktree_repo: agent.worktree_repo ?? null,
    worktree_ref: agent.worktree_ref ?? null,
    code_mode: agent.code_mode,
    agent_loop_enabled: agent.agent_loop_enabled ?? false,
    agent_loop_idle_seconds: agent.agent_loop_idle_seconds ?? null,
    agent_loop_prompt: agent.agent_loop_prompt ?? null,
  };
}

function buildDefaultWorkflowSteps(
  coordinatorMemberId: string,
  workerMemberIds: string[]
): TeamStepDraft[] {
  if (!coordinatorMemberId.trim()) {
    return [];
  }
  const planningStep: TeamStepDraft = {
    step_key: "coordinator_plan",
    member_id: coordinatorMemberId,
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
    step_key: "coordinator_synthesize",
    member_id: coordinatorMemberId,
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

export function buildCoordinatorForgeDefaultWorkdir(
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
  const nameToken = normalizedName || "coordinator";
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
    const symptom = new Error(`${field} must be valid JSON (${detail})`) as Error & {
      cause?: unknown;
    };
    symptom.cause = err;
    throw symptom;
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
