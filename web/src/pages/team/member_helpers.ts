import type { TeamPromptDefaultsRecord } from "../../api";
import { AgentRecord, TeamRunSnapshotRecord, TeamRuntimeMemberRecord } from "../../api";
import { isAgentActiveStatus } from "../../agent_ws";

export const DEFAULT_TEAM_COORDINATOR_SKILLS = [
  "agenthub-actor-runtime",
  "team-agents-index",
  "team-coordinator-agents-index",
  "team-coordinator-orchestrator",
  "team-actor-mailbox",
];

export const DEFAULT_TEAM_WORKER_SKILLS = [
  "agenthub-actor-runtime",
  "team-agents-index",
  "team-worker-agents-index",
  "team-worker-executor",
  "team-actor-mailbox",
];

export const REQUIRED_TEAM_COORDINATOR_SKILLS = [
  "agenthub-actor-runtime",
  "team-agents-index",
  "team-coordinator-agents-index",
  "team-coordinator-orchestrator",
  "team-actor-mailbox",
];

export const REQUIRED_TEAM_WORKER_SKILLS = [
  "agenthub-actor-runtime",
  "team-agents-index",
  "team-worker-agents-index",
  "team-worker-executor",
  "team-actor-mailbox",
];

const MANDATORY_TEAM_SKILLS = ["agenthub-actor-runtime"];
const OPTIONAL_TEAM_SKILLS = ["team-deliberation-rules"];

export const TEAM_SKILL_OPTIONS = [
  ...new Set([
    ...DEFAULT_TEAM_COORDINATOR_SKILLS,
    ...DEFAULT_TEAM_WORKER_SKILLS,
    ...OPTIONAL_TEAM_SKILLS,
  ]),
];

export type WorkerDraft = {
  member_id: string;
  description: string;
  model: string;
  prompt: string;
  skills: string[];
  custom_skills: string;
};

export type TeamSpecMember = {
  member_id: string;
  role: string;
};

export type TeamMemberAgentStatus = {
  member_id: string;
  role: string;
  agent_name?: string;
  status: string;
  missing_agent: boolean;
};

export type TeamMemberAgentStatusSummary = {
  active: number;
  inactive: number;
  missing: number;
  total: number;
};

export type TeamMemberLiveState = {
  member_id: string;
  role: string;
  agent_name?: string;
  lifecycle_status: string;
  lifecycle_tone: "active" | "inactive" | "missing";
  run_status: string;
  step_status: string;
  pending_inbox_count: number | null;
  current_work: string;
};

export type TeamCreateDraftState = {
  coordinatorMemberId: string;
  coordinatorModel: string;
  coordinatorPrompt: string;
  coordinatorSkills: string[];
  coordinatorCustomSkills: string;
  workers: WorkerDraft[];
  useSpecOverride: boolean;
  newTeamSpec: string;
  teamForgeAgentIds: string[];
};

export const EMPTY_TEAM_PROMPT_DEFAULTS: TeamPromptDefaultsRecord = {
  coordinator_prompt: "",
  worker_prompt: "",
};

export function resolveTeamPromptForRole(
  promptDefaults: TeamPromptDefaultsRecord,
  role: string
): string {
  return role === "coordinator" ? promptDefaults.coordinator_prompt : promptDefaults.worker_prompt;
}

export function selectTeamForgeAgents(
  agents: AgentRecord[],
  teamForgeAgentIds: string[]
): AgentRecord[] {
  if (teamForgeAgentIds.length === 0) {
    return [];
  }
  const byId = new Map(agents.map((agent) => [agent.id, agent]));
  return teamForgeAgentIds
    .map((agentId) => byId.get(agentId))
    .filter((agent): agent is AgentRecord => Boolean(agent));
}

export function createInitialTeamDraftState(
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): TeamCreateDraftState {
  return {
    coordinatorMemberId: "",
    coordinatorModel: "",
    coordinatorPrompt: promptDefaults.coordinator_prompt,
    coordinatorSkills: [...DEFAULT_TEAM_COORDINATOR_SKILLS],
    coordinatorCustomSkills: "",
    workers: [],
    useSpecOverride: false,
    newTeamSpec: "{}",
    teamForgeAgentIds: [],
  };
}

function parseCsvList(raw: string): string[] {
  return raw
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function ensureMandatorySkills(skills: string[], requiredSkills: string[]): string[] {
  const deduped = [...new Set(skills.map((item) => item.trim()).filter(Boolean))];
  const required = [...new Set(requiredSkills.map((item) => item.trim()).filter(Boolean))];
  const mandatory = required.filter((item) => !deduped.includes(item));
  if (mandatory.length === 0) {
    return deduped;
  }
  return [...mandatory, ...deduped];
}

export function normalizeSkillSelection(
  selected: string[],
  customRaw: string,
  fallback: string[],
  requiredSkills: string[] = MANDATORY_TEAM_SKILLS
): string[] {
  const allowed = new Set(TEAM_SKILL_OPTIONS);
  const selectedSkills = [...new Set(selected.map((item) => item.trim()).filter(Boolean))].filter(
    (item) => allowed.has(item)
  );
  const customSkills = parseCsvList(customRaw);
  const merged = [...new Set([...selectedSkills, ...customSkills])];
  if (merged.length > 0) {
    return ensureMandatorySkills(merged, requiredSkills);
  }
  return ensureMandatorySkills(fallback, requiredSkills);
}

export function toggleSkillSelection(
  selected: string[],
  skill: string,
  requiredSkills: string[] = MANDATORY_TEAM_SKILLS
): string[] {
  const normalized = skill.trim();
  if (!normalized || !TEAM_SKILL_OPTIONS.includes(normalized)) {
    return selected;
  }
  const required = new Set(
    requiredSkills.map((item) => item.trim()).filter((item) => item.length > 0)
  );
  const normalizedSelected = ensureMandatorySkills(selected, [...required]);
  if (normalizedSelected.includes(normalized)) {
    if (required.has(normalized)) {
      return normalizedSelected;
    }
    return normalizedSelected.filter((item) => item !== normalized);
  }
  return ensureMandatorySkills([...normalizedSelected, normalized], [...required]);
}

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function normalizeLifecycleStatus(status: string | null | undefined): string {
  const normalized = status?.trim().toLowerCase();
  return normalized && normalized.length > 0 ? normalized : "unknown";
}

export function parseTeamSpecMembers(spec: unknown): TeamSpecMember[] {
  const specRecord = asObjectRecord(spec);
  if (!specRecord) return [];
  const membersRaw = specRecord.members;
  if (!Array.isArray(membersRaw)) return [];

  const deduped = new Map<string, TeamSpecMember>();
  for (const item of membersRaw) {
    const memberRecord = asObjectRecord(item);
    if (!memberRecord) continue;
    const memberIdRaw = memberRecord.member_id;
    if (typeof memberIdRaw !== "string") continue;
    const memberId = memberIdRaw.trim();
    if (!memberId) continue;
    if (deduped.has(memberId)) continue;
    const roleRaw = memberRecord.role;
    const role =
      typeof roleRaw === "string" && roleRaw.trim().length > 0
        ? roleRaw.trim()
        : "member";
    deduped.set(memberId, { member_id: memberId, role });
  }
  return [...deduped.values()];
}

export function resolveTeamMemberAgentStatuses(
  spec: unknown,
  agents: AgentRecord[],
  fallbackAgentsById?: Record<string, AgentRecord | null>,
  runtimeMembers?: TeamRuntimeMemberRecord[] | null
): TeamMemberAgentStatus[] {
  return resolveTeamMemberAgentStatusesFromMembers(
    parseTeamSpecMembers(spec),
    agents,
    fallbackAgentsById,
    runtimeMembers
  );
}

export function resolveTeamMemberAgentStatusesFromMembers(
  members: TeamSpecMember[],
  agents: AgentRecord[],
  fallbackAgentsById?: Record<string, AgentRecord | null>,
  runtimeMembers?: TeamRuntimeMemberRecord[] | null
): TeamMemberAgentStatus[] {
  const byId = new Map(agents.map((agent) => [agent.id, agent]));
  const runtimeById = new Map(
    (runtimeMembers ?? []).map((member) => [member.member_id, member])
  );
  if (fallbackAgentsById) {
    for (const [memberId, agent] of Object.entries(fallbackAgentsById)) {
      if (agent && !byId.has(memberId)) {
        byId.set(memberId, agent);
      }
    }
  }
  return members.map((member) => {
    const agent = byId.get(member.member_id);
    const runtimeMember = runtimeById.get(member.member_id);
    const runtimeStatus =
      runtimeMember?.session_status?.trim() || runtimeMember?.agent_status?.trim() || "";
    if (!agent && !runtimeMember) {
      return {
        member_id: member.member_id,
        role: member.role,
        status: "missing",
        missing_agent: true,
      };
    }
    return {
      member_id: member.member_id,
      role: member.role,
      agent_name: agent?.name ?? runtimeMember?.display_name,
      status: normalizeLifecycleStatus(runtimeStatus || agent?.status || "missing"),
      missing_agent: false,
    };
  });
}

export function summarizeTeamMemberAgentStatuses(
  members: TeamMemberAgentStatus[]
): TeamMemberAgentStatusSummary {
  let active = 0;
  let missing = 0;
  for (const member of members) {
    if (member.missing_agent) {
      missing += 1;
      continue;
    }
    if (isAgentActiveStatus(member.status)) {
      active += 1;
    }
  }
  const total = members.length;
  const inactive = total - active - missing;
  return {
    active,
    inactive,
    missing,
    total,
  };
}

function resolveTeamRoleWeight(role: string): number {
  const normalized = role.trim().toLowerCase();
  if (normalized === "coordinator") return 0;
  if (normalized === "worker") return 1;
  return 2;
}

function toCompactWorkPreview(value: unknown, maxLength = 72): string {
  if (value == null) {
    return "";
  }
  const raw =
    typeof value === "string"
      ? value
      : (() => {
          try {
            return JSON.stringify(value);
          } catch {
            return String(value);
          }
        })();
  const normalized = raw.replace(/\s+/g, " ").trim();
  if (!normalized) {
    return "";
  }
  if (normalized.length <= maxLength) {
    return normalized;
  }
  return `${normalized.slice(0, maxLength - 3)}...`;
}

function resolveTeamMemberCurrentWork(
  snapshotMember?: TeamRunSnapshotRecord["members"][number]
): string {
  if (!snapshotMember) {
    return "No active run context.";
  }
  const step = snapshotMember.latest_step;
  if (!step) {
    return `run_status=${snapshotMember.status}`;
  }
  const stepLabel = step.step_key || step.id;
  const payloadPreview =
    toCompactWorkPreview(step.input) ||
    toCompactWorkPreview(step.output) ||
    toCompactWorkPreview(step.error_text);
  if (!payloadPreview) {
    return `${stepLabel} (${step.status})`;
  }
  return `${stepLabel}: ${payloadPreview}`;
}

export function resolveTeamMemberLifecycleTone(
  member: TeamMemberAgentStatus
): "active" | "inactive" | "missing" {
  if (member.missing_agent) {
    return "missing";
  }
  return isAgentActiveStatus(member.status) ? "active" : "inactive";
}

export function buildTeamMemberLiveStates(
  members: TeamMemberAgentStatus[],
  snapshotMembers?: TeamRunSnapshotRecord["members"]
): TeamMemberLiveState[] {
  const snapshotByMemberId = new Map(
    (snapshotMembers ?? []).map((member) => [member.member_id, member])
  );
  return [...members]
    .map((member) => {
      const snapshotMember = snapshotByMemberId.get(member.member_id);
      return {
        member_id: member.member_id,
        role: member.role,
        agent_name: member.agent_name,
        lifecycle_status: member.status,
        lifecycle_tone: resolveTeamMemberLifecycleTone(member),
        run_status: snapshotMember?.status ?? "-",
        step_status: snapshotMember?.latest_step?.status ?? "-",
        pending_inbox_count: snapshotMember?.pending_inbox_count ?? null,
        current_work: resolveTeamMemberCurrentWork(snapshotMember),
      };
    })
    .sort((a, b) => {
      const roleGap = resolveTeamRoleWeight(a.role) - resolveTeamRoleWeight(b.role);
      if (roleGap !== 0) return roleGap;
      return a.member_id.localeCompare(b.member_id);
    });
}

export function buildDefaultWorkerDraft(
  memberId: string,
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): WorkerDraft {
  return {
    member_id: memberId,
    description: "",
    model: "",
    prompt: promptDefaults.worker_prompt,
    skills: [...DEFAULT_TEAM_WORKER_SKILLS],
    custom_skills: "",
  };
}

export function backfillEmptyWorkerDraftPrompts(
  workers: WorkerDraft[],
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): WorkerDraft[] {
  const defaultPrompt = promptDefaults.worker_prompt.trim();
  if (!defaultPrompt) {
    return workers;
  }
  let changed = false;
  const nextWorkers = workers.map((worker) => {
    if (worker.prompt.trim()) {
      return worker;
    }
    changed = true;
    return { ...worker, prompt: defaultPrompt };
  });
  return changed ? nextWorkers : workers;
}

export function assignCreatedWorkerToDraft(
  workers: WorkerDraft[],
  createdMemberId: string,
  promptDefaults: TeamPromptDefaultsRecord = EMPTY_TEAM_PROMPT_DEFAULTS
): WorkerDraft[] {
  const memberId = createdMemberId.trim();
  if (!memberId) {
    return workers;
  }
  if (workers.some((worker) => worker.member_id.trim() === memberId)) {
    return workers;
  }
  const firstUnassigned = workers.findIndex(
    (worker) => worker.member_id.trim().length === 0
  );
  if (firstUnassigned >= 0) {
    return workers.map((worker, index) =>
      index === firstUnassigned ? { ...worker, member_id: memberId } : worker
    );
  }
  return [...workers, buildDefaultWorkerDraft(memberId, promptDefaults)];
}
