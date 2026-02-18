import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AgentRecord,
  AgentEvent,
  api,
  TeamActorMessageRecord,
  TeamDefinitionRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamRunStatus,
  TeamRunSnapshotRecord,
  TeamStepRecord,
} from "../api";
import {
  DEFAULT_AGENT_PRESET_ID,
  formatAgentModelLabel,
  getAgentPreset,
  isAgentPresetId,
  listAgentPresets,
  type AgentPresetId,
} from "../agent_presets";
import { isAgentActiveStatus } from "../agent_ws";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";

type TeamPageProps = {
  auth: AuthState;
  token: string;
  onLogout: () => void;
};

type TeamTab = "overview" | "events" | "steps" | "mailbox" | "member_console";
type CreateTeamStage = 0 | 1 | 2 | 3;
type TeamRunStatusFilter = TeamRunStatus | "all";
type TeamRunBrowserState = {
  statusFilter: TeamRunStatusFilter;
  beforeCreatedAt?: number;
  hasMore: boolean;
};
type MailboxTemplateKey =
  | "leader_task_assignment"
  | "clarification_request"
  | "clarification_response"
  | "worker_done"
  | "worker_blocked"
  | "profile_patch_proposal";
type StepAction =
  | "start"
  | "complete"
  | "fail"
  | "input_required"
  | "resume";
type TeamForgeBindTarget = "none" | "leader" | "worker";

const EVENT_PAGE_LIMIT = 100;
const MEMBER_EVENT_PAGE_LIMIT = 300;
const TEAM_RUN_PAGE_LIMIT = 50;
const TEAM_EVENT_PREVIEW_LIMIT = 5;
const DEFAULT_TEAM_RUN_BROWSER_STATE: TeamRunBrowserState = {
  statusFilter: "all",
  hasMore: false,
};
const MAILBOX_TEMPLATE_OPTIONS: Array<{
  value: MailboxTemplateKey;
  label: string;
}> = [
  { value: "leader_task_assignment", label: "Leader Task Assignment" },
  { value: "clarification_request", label: "Clarification Request" },
  { value: "clarification_response", label: "Clarification Response" },
  { value: "worker_done", label: "Worker Done Status" },
  { value: "worker_blocked", label: "Worker Blocked Status" },
  { value: "profile_patch_proposal", label: "Profile Patch Proposal" },
];
const TEAM_RUN_STATUS_FILTER_OPTIONS: Array<{
  value: TeamRunStatusFilter;
  label: string;
}> = [
  { value: "all", label: "All statuses" },
  { value: "submitted", label: "submitted" },
  { value: "working", label: "working" },
  { value: "input_required", label: "input_required" },
  { value: "completed", label: "completed" },
  { value: "failed", label: "failed" },
  { value: "canceled", label: "canceled" },
];
const CREATE_TEAM_STAGE_TITLES = [
  "Mission Brief",
  "Leader Forge",
  "Recruit Workers",
  "Launch Team",
] as const;
const DEFAULT_TEAM_LEADER_PROMPT = [
  "You are the Team Leader in AgentHub.",
  "Your job is to plan, delegate work to workers, and synthesize the final answer.",
  "Workflow:",
  "1. Read the run input and create a concise execution plan.",
  "2. Use actor mailbox to assign concrete tasks to workers.",
  "3. Pull inbox regularly and acknowledge consumed messages.",
  "4. Merge worker outputs, resolve conflicts, and produce final deliverable.",
  "5. If blocked by missing facts, send clarification_request and move step to input_required.",
  "Structured payload contracts:",
  "- leader_task_assignment: {\"type\":\"leader_task_assignment\",\"task\":\"...\",\"acceptance\":\"...\",\"deadline\":\"...\"}",
  "- clarification_request: {\"type\":\"clarification_request\",\"question\":\"...\",\"choices\":[\"...\"],\"blocking_scope\":\"run|step\",\"context\":{}}",
  "- profile_patch_proposal: {\"type\":\"profile_patch_proposal\",\"target\":\"run|team\",\"prompt_append\":\"...\",\"skills_add\":[\"...\"]}",
].join("\n");
const DEFAULT_TEAM_WORKER_PROMPT = [
  "You are a Worker in an AgentHub team.",
  "Your job is to execute assignments from the team leader and report results.",
  "Workflow:",
  "1. Pull inbox and find the latest task from leader.",
  "2. Acknowledge messages after reading.",
  "3. Execute the task and summarize output with evidence.",
  "4. Send the result back to leader via actor mailbox.",
  "5. If blocked, send blocker details and a proposed next action.",
  "Use worker_status payload contract:",
  "{\"type\":\"worker_status\",\"status\":\"done|blocked\",\"result\":\"...\",\"evidence\":[\"...\"],\"next_action\":\"...\"}",
].join("\n");
const DEFAULT_TEAM_LEADER_SKILLS = ["agenthub-actor-runtime", "team-leader-orchestrator"];
const DEFAULT_TEAM_WORKER_SKILLS = ["agenthub-actor-runtime", "team-worker-executor"];
const MANDATORY_TEAM_SKILLS = ["agenthub-actor-runtime"];
const TEAM_SKILL_OPTIONS = [
  ...new Set([...DEFAULT_TEAM_LEADER_SKILLS, ...DEFAULT_TEAM_WORKER_SKILLS]),
];
const TEAM_MODEL_PRESET_OPTIONS = listAgentPresets().map((preset) => ({
  value: preset.id,
  label: preset.label,
}));
const TEAM_MODEL_PRESET_VALUES = new Set(
  TEAM_MODEL_PRESET_OPTIONS.map((option) => option.value)
);

type WorkerDraft = {
  member_id: string;
  model: string;
  prompt: string;
  skills: string[];
  custom_skills: string;
};

type TeamStepDraft = {
  step_key: string;
  member_id: string;
  depends_on: string[];
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

function sortRuns(runs: TeamRunRecord[]): TeamRunRecord[] {
  return [...runs].sort((a, b) => b.created_at - a.created_at);
}

function upsertRun(list: TeamRunRecord[], nextRun: TeamRunRecord): TeamRunRecord[] {
  const withoutCurrent = list.filter((run) => run.id !== nextRun.id);
  return sortRuns([nextRun, ...withoutCurrent]);
}

export function mergeRunPages(
  existing: TeamRunRecord[],
  incoming: TeamRunRecord[]
): TeamRunRecord[] {
  const byId = new Map<string, TeamRunRecord>();
  for (const run of existing) {
    byId.set(run.id, run);
  }
  for (const run of incoming) {
    byId.set(run.id, run);
  }
  return sortRuns([...byId.values()]);
}

export function mergeTeamRunList(
  previousTeamRuns: TeamRunRecord[],
  incoming: TeamRunRecord[],
  mode: "replace" | "append",
  activeRunId: string | null
): TeamRunRecord[] {
  const base = mode === "append" ? previousTeamRuns : [];
  let merged = mergeRunPages(base, incoming);
  if (mode !== "replace" || !activeRunId) {
    return merged;
  }
  const pinned = previousTeamRuns.find((run) => run.id === activeRunId);
  if (!pinned || merged.some((run) => run.id === pinned.id)) {
    return merged;
  }
  merged = mergeRunPages(merged, [pinned]);
  return merged;
}

export function resolveRunStatusFilter(
  status: TeamRunStatusFilter
): TeamRunStatus | undefined {
  return status === "all" ? undefined : status;
}

export function selectTeamPreviewEvents(
  events: TeamRunEventRecord[],
  selectedMemberId: string,
  limit = TEAM_EVENT_PREVIEW_LIMIT
): TeamRunEventRecord[] {
  if (selectedMemberId.trim().length > 0) {
    return events;
  }
  if (events.length <= limit) {
    return events;
  }
  return events.slice(events.length - limit);
}

function upsertEventList(
  prev: TeamRunEventRecord[],
  next: TeamRunEventRecord[],
  mode: "replace" | "prepend"
): TeamRunEventRecord[] {
  const merged = mode === "replace" ? [...next] : [...next, ...prev];
  const byId = new Map<number, TeamRunEventRecord>();
  for (const event of merged) {
    byId.set(event.event_id, event);
  }
  return [...byId.values()].sort((a, b) => a.event_id - b.event_id);
}

function upsertAgentEventList(
  prev: AgentEvent[],
  next: AgentEvent[],
  mode: "replace" | "prepend"
): AgentEvent[] {
  const merged = mode === "replace" ? [...next] : [...next, ...prev];
  const byId = new Map<number, AgentEvent>();
  for (const event of merged) {
    byId.set(event.event_id, event);
  }
  return [...byId.values()].sort((a, b) => a.event_id - b.event_id);
}

function buildTeamSpecFromForm(
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
        DEFAULT_TEAM_WORKER_SKILLS
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
        DEFAULT_TEAM_LEADER_SKILLS
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

function parseErrorMessage(err: unknown): string {
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

function parseRequiredJson(raw: string, field: string): unknown {
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

function parseOptionalJson(raw: string, field: string): unknown | undefined {
  const trimmed = raw.trim();
  if (!trimmed) {
    return undefined;
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    throw new Error(`${field} must be valid JSON`);
  }
}

function parseOptionalInteger(raw: string, field: string): number | undefined {
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

function parseCsvList(raw: string): string[] {
  return raw
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function ensureMandatorySkills(skills: string[]): string[] {
  const deduped = [...new Set(skills.map((item) => item.trim()).filter(Boolean))];
  const mandatory = MANDATORY_TEAM_SKILLS.filter((item) => !deduped.includes(item));
  if (mandatory.length === 0) {
    return deduped;
  }
  return [...mandatory, ...deduped];
}

export function normalizeSkillSelection(
  selected: string[],
  customRaw: string,
  fallback: string[]
): string[] {
  const allowed = new Set(TEAM_SKILL_OPTIONS);
  const selectedSkills = [...new Set(selected.map((item) => item.trim()).filter(Boolean))].filter(
    (item) => allowed.has(item)
  );
  const customSkills = parseCsvList(customRaw);
  const merged = [...new Set([...selectedSkills, ...customSkills])];
  if (merged.length > 0) {
    return ensureMandatorySkills(merged);
  }
  return ensureMandatorySkills(fallback);
}

export function toggleSkillSelection(selected: string[], skill: string): string[] {
  const normalized = skill.trim();
  if (!normalized || !TEAM_SKILL_OPTIONS.includes(normalized)) {
    return selected;
  }
  if (selected.includes(normalized)) {
    return selected.filter((item) => item !== normalized);
  }
  return [...selected, normalized];
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
  agents: AgentRecord[]
): TeamMemberAgentStatus[] {
  const byId = new Map(agents.map((agent) => [agent.id, agent]));
  return parseTeamSpecMembers(spec).map((member) => {
    const agent = byId.get(member.member_id);
    if (!agent) {
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
      agent_name: agent.name,
      status: normalizeLifecycleStatus(agent.status),
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
  if (normalized === "leader") return 0;
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

export function buildMailboxPayloadTemplate(template: MailboxTemplateKey): unknown {
  switch (template) {
    case "leader_task_assignment":
      return {
        type: "leader_task_assignment",
        task: "Implement the requested change in a focused scope.",
        acceptance: "All listed checks pass and artifacts are updated.",
        deadline: "asap",
      };
    case "clarification_request":
      return {
        type: "clarification_request",
        question: "Need one product decision before continuing.",
        choices: ["option_a", "option_b"],
        blocking_scope: "run",
        context: {},
      };
    case "clarification_response":
      return {
        type: "clarification_response",
        request_id: "fill_request_id",
        answer: "option_a",
        rationale: "Fits current constraints and priority.",
      };
    case "worker_done":
      return {
        type: "worker_status",
        status: "done",
        result: "Implemented scoped change and verified behavior.",
        evidence: ["path/to/file:123", "test_name"],
      };
    case "worker_blocked":
      return {
        type: "worker_status",
        status: "blocked",
        result: "Blocked by missing requirement detail.",
        evidence: ["blocking_input_missing"],
        next_action: "Please provide target behavior for edge case X.",
      };
    case "profile_patch_proposal":
      return {
        type: "profile_patch_proposal",
        target: "run",
        prompt_append: "Add missing domain constraint and output contract.",
        skills_add: ["team-leader-orchestrator"],
      };
    default:
      return {};
  }
}

function buildAgentLabel(agent: AgentRecord): string {
  const model = formatAgentModelLabel(agent.command, agent.args) ?? "Unknown";
  return `${agent.name} · ${model} · ${agent.id.slice(0, 8)}`;
}

function pickNextWorkerAgentId(
  agents: AgentRecord[],
  excludedAgentIds: Set<string>
): string {
  return agents.find((agent) => !excludedAgentIds.has(agent.id))?.id ?? "";
}

function buildDefaultWorkerDraft(memberId: string): WorkerDraft {
  return {
    member_id: memberId,
    model: "",
    prompt: DEFAULT_TEAM_WORKER_PROMPT,
    skills: [...DEFAULT_TEAM_WORKER_SKILLS],
    custom_skills: "",
  };
}

export function assignCreatedWorkerToDraft(
  workers: WorkerDraft[],
  createdMemberId: string
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
  return [...workers, buildDefaultWorkerDraft(memberId)];
}

function clampCreateTeamStage(next: number): CreateTeamStage {
  if (next <= 0) return 0;
  if (next >= 3) return 3;
  return next as CreateTeamStage;
}

function resolveTeamModelOptions(currentModel: string): Array<{
  value: string;
  label: string;
}> {
  const options = [
    { value: "", label: "Use default model" },
    ...TEAM_MODEL_PRESET_OPTIONS,
  ];
  const normalized = currentModel.trim();
  if (normalized && !TEAM_MODEL_PRESET_VALUES.has(normalized)) {
    options.push({ value: normalized, label: `Custom (${normalized})` });
  }
  return options;
}

function formatTs(ts?: number | null): string {
  if (!ts) return "-";
  return new Date(ts * 1000).toLocaleString();
}

function toPrettyJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function TeamPage(props: TeamPageProps) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const [tab, setTab] = useState<TeamTab>("overview");
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [teams, setTeams] = useState<TeamDefinitionRecord[]>([]);
  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(null);

  const [newTeamName, setNewTeamName] = useState("");
  const [newTeamDescription, setNewTeamDescription] = useState("");
  const [useSpecOverride, setUseSpecOverride] = useState(false);
  const [newTeamSpec, setNewTeamSpec] = useState("{}");
  const [showCreateTeamModal, setShowCreateTeamModal] = useState(false);
  const [createTeamStage, setCreateTeamStage] = useState<CreateTeamStage>(0);
  const [leaderMemberId, setLeaderMemberId] = useState("");
  const [leaderModel, setLeaderModel] = useState("");
  const [leaderPrompt, setLeaderPrompt] = useState(DEFAULT_TEAM_LEADER_PROMPT);
  const [leaderSkills, setLeaderSkills] = useState<string[]>([
    ...DEFAULT_TEAM_LEADER_SKILLS,
  ]);
  const [leaderCustomSkills, setLeaderCustomSkills] = useState("");
  const [workers, setWorkers] = useState<WorkerDraft[]>([]);
  const [forgeAgentBindTarget, setForgeAgentBindTarget] =
    useState<TeamForgeBindTarget>("none");
  const [showForgeAgentForm, setShowForgeAgentForm] = useState(false);
  const [forgeAgentName, setForgeAgentName] = useState("");
  const [forgeAgentWorkdir, setForgeAgentWorkdir] = useState("");
  const [forgeAgentPresetId, setForgeAgentPresetId] =
    useState<AgentPresetId>(DEFAULT_AGENT_PRESET_ID);
  const [forgeAgentCodeMode, setForgeAgentCodeMode] = useState(true);
  const [forgeAgentBusy, setForgeAgentBusy] = useState(false);

  const [runContextId, setRunContextId] = useState("");
  const [runInput, setRunInput] = useState("{}");
  const [runLookupId, setRunLookupId] = useState("");

  const [runs, setRuns] = useState<TeamRunRecord[]>([]);
  const [teamRunBrowserByTeam, setTeamRunBrowserByTeam] = useState<
    Record<string, TeamRunBrowserState>
  >({});
  const [runsLoading, setRunsLoading] = useState(false);
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const activeRunIdRef = useRef<string | null>(null);
  const [snapshot, setSnapshot] = useState<TeamRunSnapshotRecord | null>(null);
  const [snapshotLoading, setSnapshotLoading] = useState(false);

  const [events, setEvents] = useState<TeamRunEventRecord[]>([]);
  const [eventsHasMore, setEventsHasMore] = useState(false);
  const [eventsLoading, setEventsLoading] = useState(false);
  const [eventsAutoRefresh, setEventsAutoRefresh] = useState(true);

  const [steps, setSteps] = useState<TeamStepRecord[]>([]);
  const [stepKey, setStepKey] = useState("");
  const [stepMemberId, setStepMemberId] = useState("");
  const [stepDependsOn, setStepDependsOn] = useState("");
  const [stepInput, setStepInput] = useState("{}");

  const [selectedStepId, setSelectedStepId] = useState<string>("");
  const [stepAction, setStepAction] = useState<StepAction>("start");
  const [stepRemoteTaskId, setStepRemoteTaskId] = useState("");
  const [stepOutput, setStepOutput] = useState("{}");
  const [stepFailText, setStepFailText] = useState("");
  const [stepInputReason, setStepInputReason] = useState("");
  const [stepInputRequiredPayload, setStepInputRequiredPayload] = useState("{}");
  const [stepResumePayload, setStepResumePayload] = useState("{}");

  const [msgFromActorId, setMsgFromActorId] = useState("");
  const [msgToActorId, setMsgToActorId] = useState("");
  const [msgChannel, setMsgChannel] = useState("default");
  const [msgTransport, setMsgTransport] = useState<"local" | "remote">("local");
  const [msgRoute, setMsgRoute] = useState("");
  const [msgTemplate, setMsgTemplate] = useState<MailboxTemplateKey>(
    "leader_task_assignment"
  );
  const [msgPayload, setMsgPayload] = useState("{}");
  const [msgIdempotencyKey, setMsgIdempotencyKey] = useState("");

  const [inboxActorId, setInboxActorId] = useState("");
  const [inboxLimit, setInboxLimit] = useState("100");
  const [inboxAfterId, setInboxAfterId] = useState("");
  const [inboxIncludeDelivered, setInboxIncludeDelivered] = useState(false);
  const [inbox, setInbox] = useState<TeamActorMessageRecord[]>([]);
  const eventsRef = useRef<TeamRunEventRecord[]>([]);
  const [selectedMemberId, setSelectedMemberId] = useState("");
  const [memberEvents, setMemberEvents] = useState<AgentEvent[]>([]);
  const [memberEventsHasMore, setMemberEventsHasMore] = useState(false);
  const [memberEventsLoading, setMemberEventsLoading] = useState(false);
  const memberEventsRef = useRef<AgentEvent[]>([]);

  const selectedTeam = useMemo(
    () => teams.find((team) => team.id === selectedTeamId) ?? null,
    [teams, selectedTeamId]
  );
  const teamMemberStatusByTeamId = useMemo(() => {
    const next = new Map<string, TeamMemberAgentStatus[]>();
    for (const team of teams) {
      next.set(team.id, resolveTeamMemberAgentStatuses(team.spec, agents));
    }
    return next;
  }, [agents, teams]);
  const teamMemberSummaryByTeamId = useMemo(() => {
    const next = new Map<string, TeamMemberAgentStatusSummary>();
    for (const team of teams) {
      const members = teamMemberStatusByTeamId.get(team.id) ?? [];
      next.set(team.id, summarizeTeamMemberAgentStatuses(members));
    }
    return next;
  }, [teamMemberStatusByTeamId, teams]);
  const selectedTeamMemberStatuses = useMemo(
    () => (selectedTeam ? teamMemberStatusByTeamId.get(selectedTeam.id) ?? [] : []),
    [selectedTeam, teamMemberStatusByTeamId]
  );
  const selectedTeamMemberSummary = useMemo(
    () => summarizeTeamMemberAgentStatuses(selectedTeamMemberStatuses),
    [selectedTeamMemberStatuses]
  );
  const selectedTeamMemberLiveStates = useMemo(
    () => buildTeamMemberLiveStates(selectedTeamMemberStatuses, snapshot?.members),
    [selectedTeamMemberStatuses, snapshot]
  );
  const leaderAgent = useMemo(
    () => agents.find((agent) => agent.id === leaderMemberId) ?? null,
    [agents, leaderMemberId]
  );
  const hasAgents = agents.length > 0;

  const activeRun = useMemo(
    () => runs.find((run) => run.id === activeRunId) ?? null,
    [runs, activeRunId]
  );
  const selectedTeamRunBrowserState = useMemo<TeamRunBrowserState>(() => {
    if (!selectedTeamId) {
      return DEFAULT_TEAM_RUN_BROWSER_STATE;
    }
    return teamRunBrowserByTeam[selectedTeamId] ?? DEFAULT_TEAM_RUN_BROWSER_STATE;
  }, [selectedTeamId, teamRunBrowserByTeam]);
  const runStatusFilter = selectedTeamRunBrowserState.statusFilter;
  const runsHasMore = selectedTeamRunBrowserState.hasMore;
  const runsBeforeCreatedAt = selectedTeamRunBrowserState.beforeCreatedAt;
  const totalLoadedRunsForTeam = useMemo(() => {
    if (!selectedTeamId) return 0;
    return runs.filter((run) => run.team_id === selectedTeamId).length;
  }, [runs, selectedTeamId]);

  const visibleRuns = useMemo(() => {
    if (!selectedTeamId) return [];
    return runs.filter((run) => {
      if (run.team_id !== selectedTeamId) return false;
      if (runStatusFilter === "all") return true;
      return run.status === runStatusFilter;
    });
  }, [runStatusFilter, runs, selectedTeamId]);
  const isActiveRunHiddenByFilter = useMemo(() => {
    if (!activeRun || !selectedTeamId) return false;
    if (activeRun.team_id !== selectedTeamId) return false;
    if (runStatusFilter === "all") return false;
    return activeRun.status !== runStatusFilter;
  }, [activeRun, runStatusFilter, selectedTeamId]);

  const builtTeamSpec = useMemo(
    () =>
      buildTeamSpecFromForm(
        leaderMemberId,
        leaderModel,
        leaderPrompt,
        leaderSkills,
        leaderCustomSkills,
        workers
      ),
    [
      leaderMemberId,
      leaderModel,
      leaderPrompt,
      leaderSkills,
      leaderCustomSkills,
      workers,
    ]
  );

  const displayedTeamSpec = useMemo(() => {
    if (useSpecOverride) {
      return newTeamSpec;
    }
    return JSON.stringify(builtTeamSpec, null, 2);
  }, [builtTeamSpec, newTeamSpec, useSpecOverride]);

  const selectedMemberSnapshot = useMemo(
    () => snapshot?.members.find((member) => member.member_id === selectedMemberId) ?? null,
    [selectedMemberId, snapshot]
  );
  const previewMode = selectedMemberId.trim().length === 0;
  const displayedRunEvents = useMemo(
    () => selectTeamPreviewEvents(events, selectedMemberId),
    [events, selectedMemberId]
  );
  const configuredWorkerCount = useMemo(
    () => workers.filter((worker) => worker.member_id.trim().length > 0).length,
    [workers]
  );
  const workerAgentIds = useMemo(
    () =>
      workers
        .map((worker) => worker.member_id.trim())
        .filter((memberId) => memberId.length > 0),
    [workers]
  );
  const selectedMemberIds = useMemo(
    () => [leaderMemberId.trim(), ...workerAgentIds].filter((item) => item.length > 0),
    [leaderMemberId, workerAgentIds]
  );
  const duplicateMemberIds = useMemo(() => {
    const counts = new Map<string, number>();
    for (const memberId of selectedMemberIds) {
      counts.set(memberId, (counts.get(memberId) ?? 0) + 1);
    }
    return [...counts.entries()]
      .filter(([, count]) => count > 1)
      .map(([memberId]) => memberId);
  }, [selectedMemberIds]);
  const hasDuplicateMembers = duplicateMemberIds.length > 0;
  const unassignedWorkerSlots = useMemo(
    () => workers.filter((worker) => worker.member_id.trim().length === 0).length,
    [workers]
  );
  const availableWorkerAgentCount = useMemo(() => {
    const used = new Set<string>([leaderMemberId.trim(), ...workerAgentIds]);
    return agents.filter((agent) => !used.has(agent.id)).length;
  }, [agents, leaderMemberId, workerAgentIds]);
  const isMissionBriefReady = useMemo(
    () => newTeamName.trim().length > 0,
    [newTeamName]
  );
  const isLeaderForgeReady = useMemo(
    () => leaderMemberId.trim().length > 0 && agents.some((agent) => agent.id === leaderMemberId),
    [agents, leaderMemberId]
  );
  const isRecruitWorkersReady = useMemo(
    () => !hasDuplicateMembers,
    [hasDuplicateMembers]
  );
  const createStageReadiness = useMemo(
    () =>
      ({
        0: isMissionBriefReady,
        1: isLeaderForgeReady,
        2: isRecruitWorkersReady,
        3: true,
      }) as Record<CreateTeamStage, boolean>,
    [isLeaderForgeReady, isMissionBriefReady, isRecruitWorkersReady]
  );
  const currentStageBlockReason = useMemo(() => {
    if (createTeamStage === 0 && !isMissionBriefReady) {
      return "Team name is required to continue.";
    }
    if (createTeamStage === 1 && !isLeaderForgeReady) {
      return "Select a valid leader agent before continuing.";
    }
    if (createTeamStage === 2 && !isRecruitWorkersReady) {
      return "Resolve duplicate member assignments before continuing.";
    }
    return null;
  }, [
    createTeamStage,
    isLeaderForgeReady,
    isMissionBriefReady,
    isRecruitWorkersReady,
  ]);
  const canAdvanceCreateStage = useMemo(() => {
    return createStageReadiness[createTeamStage];
  }, [createStageReadiness, createTeamStage]);
  const canEnterCreateStage = useCallback(
    (target: CreateTeamStage): boolean => {
      if (target <= createTeamStage) {
        return true;
      }
      for (let index = 0; index < target; index += 1) {
        const stage = index as CreateTeamStage;
        if (!createStageReadiness[stage]) {
          return false;
        }
      }
      return true;
    },
    [createStageReadiness, createTeamStage]
  );
  const questChecklist = useMemo(
    () => [
      {
        key: "brief",
        label: "Mission name set",
        ready: isMissionBriefReady,
      },
      {
        key: "leader",
        label: "Leader selected",
        ready: isLeaderForgeReady,
      },
      {
        key: "party",
        label: hasDuplicateMembers
          ? "Resolve duplicate member assignments"
          : "Party assignments are unique",
        ready: isRecruitWorkersReady,
      },
      {
        key: "launch",
        label: useSpecOverride ? "Manual spec override enabled" : "Auto workflow ready",
        ready: true,
      },
    ],
    [
      hasDuplicateMembers,
      isLeaderForgeReady,
      isMissionBriefReady,
      isRecruitWorkersReady,
      useSpecOverride,
    ]
  );
  const leaderModelOptions = useMemo(
    () => resolveTeamModelOptions(leaderModel),
    [leaderModel]
  );
  const forgeAgentPresetOptions = useMemo(
    () =>
      listAgentPresets().map((preset) => ({
        value: preset.id,
        label: preset.label,
      })),
    []
  );
  const forgeAgentPreset = useMemo(
    () => getAgentPreset(forgeAgentPresetId),
    [forgeAgentPresetId]
  );
  const leaderAgentOptions = useMemo(
    () =>
      agents.map((agent) => ({
        value: agent.id,
        label: buildAgentLabel(agent),
      })),
    [agents]
  );
  const leaderAgentSelectOptions = useMemo(() => {
    const options = [...leaderAgentOptions];
    const hasSelected = options.some((option) => option.value === leaderMemberId);
    if (leaderMemberId && !hasSelected) {
      options.unshift({
        value: leaderMemberId,
        label: `Missing agent (${leaderMemberId})`,
      });
    }
    return options;
  }, [leaderAgentOptions, leaderMemberId]);

  const oldestEventId = events.length > 0 ? events[0].event_id : null;
  const oldestMemberEventId =
    memberEvents.length > 0 ? memberEvents[0].event_id : null;

  const resetTeamDraft = useCallback((agentPool: AgentRecord[]) => {
    const leaderId = agentPool[0]?.id ?? "";
    const excluded = new Set<string>();
    if (leaderId) {
      excluded.add(leaderId);
    }
    const firstWorkerId = pickNextWorkerAgentId(agentPool, excluded);
    setNewTeamName("");
    setNewTeamDescription("");
    setLeaderMemberId(leaderId);
    setLeaderModel("");
    setLeaderPrompt(DEFAULT_TEAM_LEADER_PROMPT);
    setLeaderSkills([...DEFAULT_TEAM_LEADER_SKILLS]);
    setLeaderCustomSkills("");
    setWorkers(firstWorkerId ? [buildDefaultWorkerDraft(firstWorkerId)] : []);
    setUseSpecOverride(false);
    setNewTeamSpec("{}");
  }, []);

  const refreshAgents = useCallback(async () => {
    const list = await api.listAgents(props.token);
    setAgents(list);
    return list;
  }, [props.token]);

  const refreshTeams = useCallback(async () => {
    setBusy("refresh-teams");
    setError(null);
    try {
      const list = await api.listTeams(props.token);
      setTeams(list);
      setSelectedTeamId((prev) => {
        if (prev && list.some((team) => team.id === prev)) {
          return prev;
        }
        return list[0]?.id ?? null;
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [props.token]);

  const refreshRun = useCallback(
    async (runId: string) => {
      const run = await api.getTeamRun(props.token, runId);
      setRuns((prev) => upsertRun(prev, run));
      return run;
    },
    [props.token]
  );

  const refreshTeamRuns = useCallback(
    async (
      teamId: string,
      mode: "replace" | "append" = "replace",
      options?: {
        statusFilter?: TeamRunStatusFilter;
        beforeCreatedAt?: number;
      }
    ) => {
      setRunsLoading(true);
      try {
        const statusFilter = options?.statusFilter ?? "all";
        const beforeCreatedAt =
          mode === "append" ? options?.beforeCreatedAt : undefined;
        const list = await api.listTeamRuns(props.token, teamId, {
          limit: TEAM_RUN_PAGE_LIMIT,
          before_created_at: beforeCreatedAt,
          status: resolveRunStatusFilter(statusFilter),
        });
        setRuns((prev) => {
          const otherTeamRuns = prev.filter((run) => run.team_id !== teamId);
          const currentTeamRuns = prev.filter((run) => run.team_id === teamId);
          const merged = mergeTeamRunList(
            currentTeamRuns,
            list,
            mode,
            activeRunIdRef.current
          );
          return sortRuns([...otherTeamRuns, ...merged]);
        });
        const hasMore = list.length >= TEAM_RUN_PAGE_LIMIT;
        const nextBeforeCreatedAt =
          list.length > 0 ? list[list.length - 1]?.created_at : undefined;
        setTeamRunBrowserByTeam((prev) => {
          return {
            ...prev,
            [teamId]: {
              statusFilter,
              hasMore,
              beforeCreatedAt: hasMore ? nextBeforeCreatedAt : undefined,
            },
          };
        });
        return list;
      } finally {
        setRunsLoading(false);
      }
    },
    [props.token]
  );

  const refreshSteps = useCallback(
    async (runId: string) => {
      const list = await api.listTeamRunSteps(props.token, runId);
      setSteps(list);
      setSelectedStepId((prev) => {
        if (prev && list.some((step) => step.id === prev)) {
          return prev;
        }
        return list[0]?.id ?? "";
      });
      return list;
    },
    [props.token]
  );

  const refreshEvents = useCallback(
    async (runId: string, mode: "replace" | "prepend" = "replace") => {
      setEventsLoading(true);
      try {
        const beforeId =
          mode === "prepend" ? eventsRef.current[0]?.event_id : undefined;
        const list = await api.listTeamRunEvents(
          props.token,
          runId,
          EVENT_PAGE_LIMIT,
          beforeId
        );
        setEvents((prev) => upsertEventList(prev, list, mode));
        setEventsHasMore(list.length >= EVENT_PAGE_LIMIT);
      } finally {
        setEventsLoading(false);
      }
    },
    [props.token]
  );

  const refreshSnapshot = useCallback(
    async (runId: string) => {
      setSnapshotLoading(true);
      try {
        const next = await api.getTeamRunSnapshot(props.token, runId, {
          event_limit: 200,
          message_limit: 200,
        });
        setSnapshot(next);
        return next;
      } finally {
        setSnapshotLoading(false);
      }
    },
    [props.token]
  );

  useEffect(() => {
    eventsRef.current = events;
  }, [events]);

  useEffect(() => {
    activeRunIdRef.current = activeRunId;
  }, [activeRunId]);

  useEffect(() => {
    memberEventsRef.current = memberEvents;
  }, [memberEvents]);

  const loadInbox = useCallback(async () => {
    if (!activeRunId) return;
    const actorId = inboxActorId.trim();
    if (!actorId) {
      throw new Error("Inbox actor_id is required");
    }
    const limit = parseOptionalInteger(inboxLimit, "Inbox limit") ?? 100;
    const afterId = parseOptionalInteger(inboxAfterId, "Inbox after_id");
    const list = await api.listTeamRunInbox(props.token, activeRunId, {
      actor_id: actorId,
      limit,
      after_id: afterId,
      include_delivered: inboxIncludeDelivered,
    });
    setInbox(list);
  }, [
    activeRunId,
    inboxActorId,
    inboxAfterId,
    inboxIncludeDelivered,
    inboxLimit,
    props.token,
  ]);

  const loadMemberEvents = useCallback(
    async (mode: "replace" | "prepend" = "replace") => {
      if (!selectedMemberSnapshot) {
        setMemberEvents([]);
        setMemberEventsHasMore(false);
        return;
      }
      const memberAgentId = selectedMemberSnapshot.member_id;
      const sessionId = selectedMemberSnapshot.latest_step?.remote_task_id ?? undefined;
      if (!sessionId) {
        setMemberEvents([]);
        setMemberEventsHasMore(false);
        return;
      }

      setMemberEventsLoading(true);
      try {
        const beforeId =
          mode === "prepend" ? memberEventsRef.current[0]?.event_id : undefined;
        const list = await api.listAgentEvents(
          props.token,
          memberAgentId,
          MEMBER_EVENT_PAGE_LIMIT,
          sessionId,
          beforeId
        );
        setMemberEvents((prev) => upsertAgentEventList(prev, list, mode));
        setMemberEventsHasMore(list.length >= MEMBER_EVENT_PAGE_LIMIT);
      } finally {
        setMemberEventsLoading(false);
      }
    },
    [props.token, selectedMemberSnapshot]
  );

  useEffect(() => {
    void refreshTeams();
    void refreshAgents().catch((err) => {
      setError(parseErrorMessage(err));
    });
  }, [refreshAgents, refreshTeams]);

  useEffect(() => {
    if (!selectedTeamId) {
      setActiveRunId(null);
      setRuns([]);
      setEvents([]);
      setSteps([]);
      setInbox([]);
      setSnapshot(null);
      setSelectedMemberId("");
      setMemberEvents([]);
      return;
    }
    let canceled = false;
    const loadTeamRuns = async () => {
      try {
        setError(null);
        await refreshTeamRuns(selectedTeamId, "replace", {
          statusFilter: runStatusFilter,
        });
        if (canceled) return;
      } catch (err) {
        if (!canceled) {
          setError(parseErrorMessage(err));
        }
      }
    };
    void loadTeamRuns();
    return () => {
      canceled = true;
    };
  }, [refreshTeamRuns, selectedTeamId, runStatusFilter]);

  useEffect(() => {
    if (!selectedTeamId) return;
    setActiveRunId((prev) => {
      if (prev && runs.some((run) => run.id === prev && run.team_id === selectedTeamId)) {
        return prev;
      }
      return runs.find((run) => run.team_id === selectedTeamId)?.id ?? null;
    });
  }, [runs, selectedTeamId]);

  useEffect(() => {
    if (!activeRunId) {
      setEvents([]);
      setSteps([]);
      setInbox([]);
      setSnapshot(null);
      setSelectedMemberId("");
      setMemberEvents([]);
      return;
    }
    let canceled = false;
    const loadAll = async () => {
      try {
        setError(null);
        const run = await refreshRun(activeRunId);
        if (canceled) return;
        if (run.team_id !== selectedTeamId) {
          setSelectedTeamId(run.team_id);
        }
        await Promise.all([
          refreshSteps(activeRunId),
          refreshEvents(activeRunId),
          refreshSnapshot(activeRunId),
        ]);
      } catch (err) {
        if (!canceled) {
          setError(parseErrorMessage(err));
        }
      }
    };
    void loadAll();
    return () => {
      canceled = true;
    };
  }, [
    activeRunId,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    refreshSteps,
    selectedTeamId,
  ]);

  useEffect(() => {
    if (!activeRunId || !eventsAutoRefresh) return;
    const timer = window.setInterval(() => {
      void refreshRun(activeRunId).catch(() => undefined);
      void refreshEvents(activeRunId).catch(() => undefined);
      void refreshSnapshot(activeRunId).catch(() => undefined);
    }, 4000);
    return () => {
      window.clearInterval(timer);
    };
  }, [activeRunId, eventsAutoRefresh, refreshEvents, refreshRun, refreshSnapshot]);

  useEffect(() => {
    if (!snapshot) {
      setSelectedMemberId("");
      setMemberEvents([]);
      return;
    }
    setSelectedMemberId((prev) => {
      if (prev && snapshot.members.some((member) => member.member_id === prev)) {
        return prev;
      }
      return snapshot.members[0]?.member_id ?? "";
    });
  }, [snapshot]);

  useEffect(() => {
    void loadMemberEvents("replace").catch((err) => {
      setError(parseErrorMessage(err));
    });
  }, [loadMemberEvents]);

  useEffect(() => {
    if (!showCreateTeamModal) return;
    if (leaderMemberId && agents.some((agent) => agent.id === leaderMemberId)) {
      return;
    }
    const fallbackLeaderId = agents[0]?.id ?? "";
    setLeaderMemberId(fallbackLeaderId);
  }, [agents, leaderMemberId, showCreateTeamModal]);

  useEffect(() => {
    if (!showCreateTeamModal) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (busy === "create-team") return;
      event.preventDefault();
      setShowCreateTeamModal(false);
      setCreateTeamStage(0);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [busy, showCreateTeamModal]);

  const openCreateTeamModal = () => {
    setError(null);
    setCreateTeamStage(0);
    resetTeamDraft(agents);
    setShowCreateTeamModal(true);
    setShowForgeAgentForm(false);
    void refreshAgents().catch((err) => {
      setError(parseErrorMessage(err));
    });
  };

  const closeCreateTeamModal = () => {
    setShowCreateTeamModal(false);
    setCreateTeamStage(0);
    setShowForgeAgentForm(false);
  };

  const openForgeAgentForm = () => {
    setError(null);
    setShowForgeAgentForm(true);
    const target: TeamForgeBindTarget =
      createTeamStage === 1 ? "leader" : createTeamStage === 2 ? "worker" : "none";
    setForgeAgentBindTarget(target);
    const teamToken = newTeamName.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-");
    const prefix = teamToken || "team";
    const defaultName =
      target === "leader"
        ? `${prefix}-leader`
        : target === "worker"
          ? `${prefix}-worker-${Math.max(1, workers.length + 1)}`
          : `${prefix}-agent-${Math.max(1, agents.length + 1)}`;
    setForgeAgentName(defaultName);
    setForgeAgentWorkdir("");
    setForgeAgentPresetId(DEFAULT_AGENT_PRESET_ID);
    setForgeAgentCodeMode(true);
  };

  const closeForgeAgentForm = () => {
    if (forgeAgentBusy) return;
    setShowForgeAgentForm(false);
  };

  const onCreateForgeAgent = async () => {
    if (forgeAgentBusy) return;
    const name = forgeAgentName.trim() || "agent";
    const workdir = forgeAgentWorkdir.trim();
    if (!workdir) {
      setError("Forge agent workdir is required");
      return;
    }
    setForgeAgentBusy(true);
    setError(null);
    try {
      const preset = getAgentPreset(forgeAgentPresetId);
      const created = await api.createAgent(props.token, {
        name,
        workdir,
        command: preset.command,
        args: preset.args.slice(),
        worktree_mode: "use_existing",
        worktree_repo: null,
        worktree_ref: null,
        code_mode: forgeAgentCodeMode,
      });
      setAgents((prev) => [created, ...prev.filter((agent) => agent.id !== created.id)]);
      if (forgeAgentBindTarget === "leader") {
        setLeaderMemberId(created.id);
      } else if (forgeAgentBindTarget === "worker") {
        setWorkers((prev) => assignCreatedWorkerToDraft(prev, created.id));
      }
      setShowForgeAgentForm(false);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setForgeAgentBusy(false);
    }
  };

  const onSelectCreateTeamStage = (target: CreateTeamStage) => {
    if (canEnterCreateStage(target)) {
      setError(null);
      setCreateTeamStage(target);
      return;
    }
    setError("Complete previous stage requirements before advancing.");
  };

  const goToNextCreateTeamStage = () => {
    if (!canAdvanceCreateStage) {
      setError(currentStageBlockReason ?? "Complete current stage requirements first.");
      return;
    }
    setError(null);
    setCreateTeamStage((prev) => clampCreateTeamStage(prev + 1));
  };

  const goToPrevCreateTeamStage = () => {
    setError(null);
    setCreateTeamStage((prev) => clampCreateTeamStage(prev - 1));
  };

  const onCreateTeam = async () => {
    const name = newTeamName.trim();
    if (!name) {
      setError("Team name is required");
      return;
    }
    if (!useSpecOverride && !leaderMemberId.trim()) {
      setError("Leader agent is required");
      return;
    }
    if (!useSpecOverride && !agents.some((agent) => agent.id === leaderMemberId.trim())) {
      setError("Leader must be selected from existing agents");
      return;
    }
    if (!useSpecOverride && hasDuplicateMembers) {
      setError("Leader and worker agents must be unique");
      return;
    }
    setBusy("create-team");
    setError(null);
    try {
      const specPayload = useSpecOverride
        ? parseRequiredJson(newTeamSpec, "Team spec")
        : builtTeamSpec;
      const created = await api.createTeam(props.token, {
        name,
        description: newTeamDescription.trim() || undefined,
        spec: specPayload,
      });
      setTeams((prev) => [...prev, created].sort((a, b) => a.name.localeCompare(b.name)));
      setSelectedTeamId(created.id);
      resetTeamDraft(agents);
      closeCreateTeamModal();
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onCreateRun = async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    setBusy("create-run");
    setError(null);
    try {
      const created = await api.createTeamRun(props.token, selectedTeamId, {
        context_id: runContextId.trim() || undefined,
        input: parseOptionalJson(runInput, "Run input") ?? {},
      });
      setRuns((prev) => upsertRun(prev, created));
      setActiveRunId(created.id);
      setRunLookupId(created.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onDeleteTeam = async () => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    const confirmed = window.confirm(
      `Delete team "${selectedTeam.name}" and all associated runs/events/messages?`
    );
    if (!confirmed) {
      return;
    }

    setBusy("delete-team");
    setError(null);
    try {
      await api.deleteTeam(props.token, selectedTeam.id);

      const remainingTeams = teams.filter((team) => team.id !== selectedTeam.id);
      const remainingRuns = runs.filter((run) => run.team_id !== selectedTeam.id);

      setTeams(remainingTeams);
      setRuns(remainingRuns);
      setTeamRunBrowserByTeam((prev) => {
        const next = { ...prev };
        delete next[selectedTeam.id];
        return next;
      });
      setSelectedTeamId((current) =>
        current === selectedTeam.id ? (remainingTeams[0]?.id ?? null) : current
      );
      setActiveRunId((current) =>
        current && remainingRuns.some((run) => run.id === current) ? current : null
      );
      setRunLookupId("");
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onLoadRunById = async () => {
    const runId = runLookupId.trim();
    if (!runId) {
      setError("Run ID is required");
      return;
    }
    setBusy("load-run");
    setError(null);
    try {
      const run = await refreshRun(runId);
      setSelectedTeamId(run.team_id);
      setActiveRunId(run.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onLoadMoreRuns = useCallback(async () => {
    if (!selectedTeamId || runsLoading || !runsHasMore) {
      return;
    }
    setError(null);
    try {
      await refreshTeamRuns(selectedTeamId, "append", {
        statusFilter: runStatusFilter,
        beforeCreatedAt: runsBeforeCreatedAt,
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [
    refreshTeamRuns,
    runStatusFilter,
    runsBeforeCreatedAt,
    runsHasMore,
    runsLoading,
    selectedTeamId,
  ]);

  const onCancelRun = async () => {
    if (!activeRunId) return;
    setBusy("cancel-run");
    setError(null);
    try {
      const canceled = await api.cancelTeamRun(props.token, activeRunId);
      setRuns((prev) => upsertRun(prev, canceled));
      await Promise.all([refreshEvents(activeRunId), refreshSnapshot(activeRunId)]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onSubmitStep = async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    if (!stepKey.trim()) {
      setError("step_key is required");
      return;
    }
    if (!stepMemberId.trim()) {
      setError("member_id is required");
      return;
    }
    setBusy("submit-step");
    setError(null);
    try {
      const created = await api.submitTeamRunStep(props.token, activeRunId, {
        step_key: stepKey.trim(),
        member_id: stepMemberId.trim(),
        depends_on: parseCsvList(stepDependsOn),
        input: parseOptionalJson(stepInput, "Step input"),
      });
      await Promise.all([
        refreshRun(activeRunId),
        refreshSteps(activeRunId),
        refreshEvents(activeRunId),
        refreshSnapshot(activeRunId),
      ]);
      setSelectedStepId(created.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onApplyStepAction = async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    if (!selectedStepId) {
      setError("Select a step first");
      return;
    }
    setBusy(`step-${stepAction}`);
    setError(null);
    try {
      if (stepAction === "start") {
        await api.startTeamRunStep(props.token, activeRunId, selectedStepId, {
          remote_task_id: stepRemoteTaskId.trim() || undefined,
        });
      } else if (stepAction === "complete") {
        await api.completeTeamRunStep(props.token, activeRunId, selectedStepId, {
          output: parseOptionalJson(stepOutput, "Step output"),
        });
      } else if (stepAction === "fail") {
        const errorText = stepFailText.trim();
        if (!errorText) {
          throw new Error("Fail reason is required");
        }
        await api.failTeamRunStep(props.token, activeRunId, selectedStepId, {
          error_text: errorText,
        });
      } else if (stepAction === "input_required") {
        await api.setTeamRunStepInputRequired(props.token, activeRunId, selectedStepId, {
          reason: stepInputReason.trim() || undefined,
          input: parseOptionalJson(stepInputRequiredPayload, "Input required payload"),
        });
      } else {
        await api.resumeTeamRunStep(props.token, activeRunId, selectedStepId, {
          input: parseOptionalJson(stepResumePayload, "Resume payload"),
        });
      }
      await Promise.all([
        refreshRun(activeRunId),
        refreshSteps(activeRunId),
        refreshEvents(activeRunId),
        refreshSnapshot(activeRunId),
      ]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onApplyMessageTemplate = () => {
    setMsgPayload(toPrettyJson(buildMailboxPayloadTemplate(msgTemplate)));
  };

  const onSendMessage = async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    const fromActorId = msgFromActorId.trim();
    const toActorId = msgToActorId.trim();
    if (!fromActorId || !toActorId) {
      setError("from_actor_id and to_actor_id are required");
      return;
    }
    setBusy("send-message");
    setError(null);
    try {
      await api.sendTeamRunMessage(props.token, activeRunId, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: msgChannel.trim() || undefined,
        transport: msgTransport,
        route: parseOptionalJson(msgRoute, "Message route"),
        payload: parseRequiredJson(msgPayload, "Message payload"),
        idempotency_key: msgIdempotencyKey.trim() || undefined,
      });
      await Promise.all([refreshEvents(activeRunId), refreshSnapshot(activeRunId)]);
      if (inboxActorId.trim()) {
        await loadInbox();
      }
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onRefreshInbox = async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    setBusy("refresh-inbox");
    setError(null);
    try {
      await loadInbox();
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onAckMessage = async (message: TeamActorMessageRecord) => {
    if (!activeRunId) return;
    const actorId = inboxActorId.trim() || message.to_actor_id;
    setBusy(`ack-${message.message_id}`);
    setError(null);
    try {
      await api.ackTeamRunMessage(props.token, activeRunId, message.message_id, actorId);
      await Promise.all([
        loadInbox(),
        refreshEvents(activeRunId),
        refreshSnapshot(activeRunId),
      ]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onUpdateWorker = (
    index: number,
    field: "member_id" | "model" | "prompt" | "custom_skills",
    value: string
  ) => {
    setWorkers((prev) =>
      prev.map((worker, workerIndex) =>
        workerIndex === index ? { ...worker, [field]: value } : worker
      )
    );
  };

  const onToggleWorkerSkill = (index: number, skill: string) => {
    setWorkers((prev) =>
      prev.map((worker, workerIndex) =>
        workerIndex === index
          ? {
              ...worker,
              skills: toggleSkillSelection(worker.skills, skill),
            }
          : worker
      )
    );
  };

  const onAddWorker = () => {
    setWorkers((prev) => {
      const excluded = new Set<string>([
        leaderMemberId.trim(),
        ...prev
          .map((worker) => worker.member_id.trim())
          .filter((memberId) => memberId.length > 0),
      ]);
      const memberId = pickNextWorkerAgentId(agents, excluded);
      return [...prev, buildDefaultWorkerDraft(memberId)];
    });
  };

  const onAddAllRemainingWorkers = () => {
    setWorkers((prev) => {
      const used = new Set<string>([
        leaderMemberId.trim(),
        ...prev
          .map((worker) => worker.member_id.trim())
          .filter((memberId) => memberId.length > 0),
      ]);
      const next = [...prev];
      for (const agent of agents) {
        if (used.has(agent.id)) {
          continue;
        }
        used.add(agent.id);
        next.push(buildDefaultWorkerDraft(agent.id));
      }
      return next;
    });
  };

  const onResolveDuplicateWorkers = () => {
    setWorkers((prev) => {
      const used = new Set<string>();
      const leaderId = leaderMemberId.trim();
      if (leaderId) {
        used.add(leaderId);
      }
      return prev.map((worker) => {
        const memberId = worker.member_id.trim();
        if (!memberId) {
          return worker;
        }
        if (!used.has(memberId)) {
          used.add(memberId);
          return worker;
        }
        const replacement = pickNextWorkerAgentId(agents, used);
        if (!replacement) {
          return { ...worker, member_id: "" };
        }
        used.add(replacement);
        return { ...worker, member_id: replacement };
      });
    });
  };

  const onRemoveWorker = (index: number) => {
    setWorkers((prev) => prev.filter((_, workerIndex) => workerIndex !== index));
  };

  return (
    <div className="app">
      <header>
        <h1>AgentHub Teams</h1>
        <div className="session">
          <a className="icon-button" href="/" title="Back" aria-label="Back">
            <i className="bi bi-arrow-left" aria-hidden="true" />
          </a>
          <span>{props.auth.username}</span>
          <button onClick={props.onLogout}>Logout</button>
        </div>
      </header>

      {error && <ErrorBanner message={error} onClose={() => setError(null)} />}

      <section className="teams-layout">
        <aside className="card teams-sidebar">
          <div className="mode-switch">
            <a className="mode-tag" href="/">
              Agents
            </a>
            <a className="mode-tag active" href="/teams">
              Teams
            </a>
          </div>
          <div className="toolbar">
            <h2>Teams</h2>
            <button onClick={() => void refreshTeams()} disabled={busy === "refresh-teams"}>
              Refresh
            </button>
          </div>

          <div className="teams-form teams-create-launch">
            <h3>Team Forge</h3>
            <p className="muted">
              Open the creation quest to set up Leader and Workers in stages.
            </p>
            <button onClick={openCreateTeamModal}>Create Team</button>
            <div className="teams-create-launch-meta mono">
              <span>draft_team={newTeamName.trim() || "-"}</span>
              <span>leader={leaderMemberId.trim() || "-"}</span>
              <span>workers={configuredWorkerCount}</span>
            </div>
          </div>

          <div className="teams-list">
            {teams.length === 0 && <p className="muted">No teams yet.</p>}
            {teams.map((team) => {
              const summary = teamMemberSummaryByTeamId.get(team.id);
              return (
                <button
                  key={team.id}
                  className={team.id === selectedTeamId ? "team-item active" : "team-item"}
                  onClick={() => {
                    setSelectedTeamId(team.id);
                    setRunLookupId("");
                  }}
                >
                  <span className="team-name">{team.name}</span>
                  <span className="team-id mono">{team.id}</span>
                  {summary && (
                    <span className="team-id mono team-member-summary">
                      {`active=${summary.active} inactive=${summary.inactive} missing=${summary.missing} total=${summary.total}`}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </aside>

        <div className="teams-main">
          {!selectedTeam && (
            <div className="card">
              <h2>Team Workbench</h2>
              <p>Select a team from the left panel to manage runs, steps, and messages.</p>
            </div>
          )}

          {selectedTeam && (
            <>
              <div className="card">
                <div className="toolbar">
                  <h2>{selectedTeam.name}</h2>
                  <div className="actions">
                    <span className="mono">{selectedTeam.id}</span>
                    <button
                      onClick={() => void onDeleteTeam()}
                      disabled={busy === "delete-team"}
                    >
                      Delete Team
                    </button>
                  </div>
                </div>
                <div className="teams-run-create">
                  <div className="teams-member-status-panel">
                    <div className="teams-member-summary-line mono">
                      {`active=${selectedTeamMemberSummary.active} inactive=${selectedTeamMemberSummary.inactive} missing=${selectedTeamMemberSummary.missing} total=${selectedTeamMemberSummary.total}`}
                    </div>
                    {selectedTeamMemberLiveStates.length === 0 ? (
                      <p className="muted">No members declared in team spec.</p>
                    ) : (
                      <div className="teams-member-strip">
                        {selectedTeamMemberLiveStates.map((member) => {
                          const roleTone =
                            member.role.trim().toLowerCase() === "leader"
                              ? "leader"
                              : "worker";
                          return (
                            <article
                              key={`${selectedTeam.id}:${member.member_id}`}
                              className="team-member-row"
                            >
                              <div className="team-member-row-main">
                                <span className={`team-role-chip ${roleTone}`}>
                                  {member.role}
                                </span>
                                <span className="team-member-row-id mono">{member.member_id}</span>
                                <span className={`team-status-chip ${member.lifecycle_tone}`}>
                                  {member.lifecycle_status}
                                </span>
                                <span className="team-member-row-meta mono">
                                  {member.agent_name ?? "agent_not_found"}
                                </span>
                                <span className="team-member-row-meta mono">
                                  {`run=${member.run_status} step=${member.step_status} inbox=${member.pending_inbox_count ?? "-"}`}
                                </span>
                              </div>
                              <div className="team-member-workline mono">
                                {member.current_work}
                              </div>
                            </article>
                          );
                        })}
                      </div>
                    )}
                  </div>
                  <h3>Create / Load Run</h3>
                  <p className="muted">
                    <strong>Create Run</strong> starts a new execution for this team spec.
                    <br />
                    <strong>Load Run</strong> opens an existing run by `run_id` (even if it was
                    created earlier) and auto-switches to its team.
                  </p>
                  <div className="form-row">
                    <input
                      placeholder="context_id (optional, auto-generated when empty)"
                      value={runContextId}
                      onChange={(event) => setRunContextId(event.target.value)}
                    />
                    <button onClick={onCreateRun} disabled={busy === "create-run"}>
                      Create Run
                    </button>
                  </div>
                  <textarea
                    className="mono"
                    rows={4}
                    value={runInput}
                    onChange={(event) => setRunInput(event.target.value)}
                  />
                  <div className="form-row">
                    <input
                      placeholder="existing run_id"
                      value={runLookupId}
                      onChange={(event) => setRunLookupId(event.target.value)}
                    />
                    <button onClick={onLoadRunById} disabled={busy === "load-run"}>
                      Load Run
                    </button>
                  </div>
                </div>
                <div className="teams-run-list">
                  <div className="teams-run-list-head">
                    <h3>Runs</h3>
                    <div className="actions">
                      <select
                        value={runStatusFilter}
                        onChange={(event) => {
                          if (!selectedTeamId) return;
                          const nextFilter = event.target.value as TeamRunStatusFilter;
                          setTeamRunBrowserByTeam((prev) => ({
                            ...prev,
                            [selectedTeamId]: {
                              statusFilter: nextFilter,
                              beforeCreatedAt: undefined,
                              hasMore: false,
                            },
                          }));
                        }}
                        aria-label="Run status filter"
                      >
                        {TEAM_RUN_STATUS_FILTER_OPTIONS.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <button
                        onClick={() => {
                          if (!selectedTeamId) return;
                          void refreshTeamRuns(selectedTeamId, "replace", {
                            statusFilter: runStatusFilter,
                          }).catch((err) => setError(parseErrorMessage(err)));
                        }}
                        disabled={runsLoading}
                      >
                        Refresh Runs
                      </button>
                    </div>
                  </div>
                  {visibleRuns.length === 0 && (
                    <p className="muted">No runs loaded yet. Create one or load by run_id.</p>
                  )}
                  {isActiveRunHiddenByFilter && activeRun && (
                    <p className="muted">
                      Active run `{activeRun.id}` is hidden by filter `{runStatusFilter}`.
                    </p>
                  )}
                  {visibleRuns.map((run) => (
                    <button
                      key={run.id}
                      className={run.id === activeRunId ? "team-item active" : "team-item"}
                      onClick={() => setActiveRunId(run.id)}
                    >
                      <span className="team-name mono">{run.id}</span>
                      <span className="team-status">{run.status}</span>
                    </button>
                  ))}
                  <div className="teams-run-list-foot">
                    <span className="mono">
                      showing={visibleRuns.length} loaded={totalLoadedRunsForTeam}
                      {" "}limit={TEAM_RUN_PAGE_LIMIT}
                    </span>
                    <button
                      onClick={onLoadMoreRuns}
                      disabled={runsLoading || !runsHasMore || !selectedTeamId}
                    >
                      {runsLoading ? "Loading..." : runsHasMore ? "Load More" : "No More Runs"}
                    </button>
                  </div>
                </div>
              </div>

              {activeRun && (
                <>
                  <div className="card">
                    <div className="toolbar">
                      <h3>Active Run</h3>
                      <div className="actions">
                        <button
                          onClick={() => {
                            if (!activeRunId) return;
                            void refreshRun(activeRunId).catch((err) =>
                              setError(parseErrorMessage(err))
                            );
                          }}
                        >
                          Refresh Run
                        </button>
                        <button
                          onClick={onCancelRun}
                          disabled={busy === "cancel-run" || activeRun.status === "canceled"}
                        >
                          Cancel Run
                        </button>
                      </div>
                    </div>
                    <div className="teams-run-meta">
                      <span>
                        <strong>ID:</strong> <code>{activeRun.id}</code>
                      </span>
                      <span>
                        <strong>Status:</strong> {activeRun.status}
                      </span>
                      <span>
                        <strong>Context:</strong> {activeRun.context_id}
                      </span>
                      <span>
                        <strong>Created:</strong> {formatTs(activeRun.created_at)}
                      </span>
                      <span>
                        <strong>Started:</strong> {formatTs(activeRun.started_at)}
                      </span>
                      <span>
                        <strong>Ended:</strong> {formatTs(activeRun.ended_at)}
                      </span>
                    </div>
                  </div>

                  <div className="tab-bar">
                    <button
                      className={tab === "overview" ? "tab active" : "tab"}
                      onClick={() => setTab("overview")}
                    >
                      Overview
                    </button>
                    <button
                      className={tab === "events" ? "tab active" : "tab"}
                      onClick={() => setTab("events")}
                    >
                      Events
                    </button>
                    <button
                      className={tab === "steps" ? "tab active" : "tab"}
                      onClick={() => setTab("steps")}
                    >
                      Steps
                    </button>
                    <button
                      className={tab === "mailbox" ? "tab active" : "tab"}
                      onClick={() => setTab("mailbox")}
                    >
                      Mailbox
                    </button>
                    <button
                      className={tab === "member_console" ? "tab active" : "tab"}
                      onClick={() => setTab("member_console")}
                    >
                      Member Console
                    </button>
                  </div>

                  {tab === "overview" && (
                    <div className="card">
                      <div className="toolbar">
                        <h3>Team Snapshot</h3>
                        <div className="actions">
                          <button
                            onClick={() => {
                              if (!activeRunId) return;
                              void refreshSnapshot(activeRunId).catch((err) =>
                                setError(parseErrorMessage(err))
                              );
                            }}
                            disabled={snapshotLoading}
                          >
                            Refresh Snapshot
                          </button>
                        </div>
                      </div>

                      {!snapshot && <p className="muted">No snapshot yet.</p>}

                      {snapshot && (
                        <>
                          <div className="teams-run-meta">
                            <span>
                              <strong>Leader:</strong>{" "}
                              <code>{snapshot.leader_member_id ?? "-"}</code>
                            </span>
                            <span>
                              <strong>Members:</strong> {snapshot.members.length}
                            </span>
                            <span>
                              <strong>Pending Mailbox:</strong> {snapshot.mailbox.pending}
                            </span>
                            <span>
                              <strong>Delivered:</strong> {snapshot.mailbox.delivered}
                            </span>
                            <span>
                              <strong>Dead Letter:</strong> {snapshot.mailbox.dead_letter}
                            </span>
                            <span>
                              <strong>Recent Events:</strong>{" "}
                              {snapshot.latest_events.length}
                            </span>
                          </div>

                          <div className="teams-member-list">
                            {snapshot.members.map((member) => (
                              <button
                                key={member.member_id}
                                className={
                                  selectedMemberId === member.member_id
                                    ? "team-item active"
                                    : "team-item"
                                }
                                onClick={() => {
                                  setSelectedMemberId(member.member_id);
                                  setTab("member_console");
                                }}
                              >
                                <span className="team-name">
                                  {member.member_id} ({member.role})
                                </span>
                                <span className="team-status">{member.status}</span>
                                <span className="team-id mono">
                                  {`model=${member.model ?? "-"} pending=${member.pending_inbox_count}`}
                                </span>
                              </button>
                            ))}
                          </div>
                        </>
                      )}
                    </div>
                  )}

                  {tab === "events" && (
                    <div className="card">
                      <div className="toolbar">
                        <h3>Run Events</h3>
                        <div className="actions">
                          <label className="checkbox">
                            <input
                              type="checkbox"
                              checked={eventsAutoRefresh}
                              onChange={(event) =>
                                setEventsAutoRefresh(event.target.checked)
                              }
                            />
                            Auto refresh
                          </label>
                          <button
                            onClick={() => void refreshEvents(activeRun.id)}
                            disabled={eventsLoading}
                          >
                            Refresh
                          </button>
                          <button
                            onClick={() => void refreshEvents(activeRun.id, "prepend")}
                            disabled={
                              previewMode ||
                              eventsLoading ||
                              !eventsHasMore ||
                              oldestEventId == null
                            }
                          >
                            Load Older
                          </button>
                        </div>
                      </div>
                      {previewMode && (
                        <p className="muted">
                          Showing latest {TEAM_EVENT_PREVIEW_LIMIT} records. For full event
                          history, select a member in the Member Console tab.
                        </p>
                      )}
                      {displayedRunEvents.length === 0 && <p className="muted">No events.</p>}
                      <ul className="teams-event-list">
                        {displayedRunEvents.map((event) => (
                          <li key={event.event_id}>
                            <div className="teams-event-head">
                              <span className="mono">#{event.event_id}</span>
                              <span>{event.event_type}</span>
                              <span>{formatTs(event.ts)}</span>
                            </div>
                            <pre className="mono">{toPrettyJson(event.payload)}</pre>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {tab === "steps" && (
                    <div className="card">
                      <div className="toolbar">
                        <h3>Steps</h3>
                        <button onClick={() => void refreshSteps(activeRun.id)}>Refresh</button>
                      </div>

                      <div className="teams-step-grid">
                        <div className="teams-step-panel">
                          <h4>Submit Step</h4>
                          <input
                            placeholder="step_key"
                            value={stepKey}
                            onChange={(event) => setStepKey(event.target.value)}
                          />
                          <input
                            placeholder="member_id"
                            value={stepMemberId}
                            onChange={(event) => setStepMemberId(event.target.value)}
                          />
                          <input
                            placeholder="depends_on (comma separated)"
                            value={stepDependsOn}
                            onChange={(event) => setStepDependsOn(event.target.value)}
                          />
                          <textarea
                            className="mono"
                            rows={4}
                            value={stepInput}
                            onChange={(event) => setStepInput(event.target.value)}
                          />
                          <button onClick={onSubmitStep} disabled={busy === "submit-step"}>
                            Submit Step
                          </button>
                        </div>

                        <div className="teams-step-panel">
                          <h4>Step Action</h4>
                          <select
                            value={selectedStepId}
                            onChange={(event) => setSelectedStepId(event.target.value)}
                          >
                            <option value="">Select step</option>
                            {steps.map((step) => (
                              <option key={step.id} value={step.id}>
                                {step.step_key} ({step.status})
                              </option>
                            ))}
                          </select>
                          <select
                            value={stepAction}
                            onChange={(event) =>
                              setStepAction(event.target.value as StepAction)
                            }
                          >
                            <option value="start">start</option>
                            <option value="complete">complete</option>
                            <option value="fail">fail</option>
                            <option value="input_required">input_required</option>
                            <option value="resume">resume</option>
                          </select>

                          {stepAction === "start" && (
                            <input
                              placeholder="remote_task_id (optional)"
                              value={stepRemoteTaskId}
                              onChange={(event) =>
                                setStepRemoteTaskId(event.target.value)
                              }
                            />
                          )}

                          {stepAction === "complete" && (
                            <textarea
                              className="mono"
                              rows={4}
                              value={stepOutput}
                              onChange={(event) => setStepOutput(event.target.value)}
                            />
                          )}

                          {stepAction === "fail" && (
                            <input
                              placeholder="error_text"
                              value={stepFailText}
                              onChange={(event) => setStepFailText(event.target.value)}
                            />
                          )}

                          {stepAction === "input_required" && (
                            <>
                              <input
                                placeholder="reason (optional)"
                                value={stepInputReason}
                                onChange={(event) =>
                                  setStepInputReason(event.target.value)
                                }
                              />
                              <textarea
                                className="mono"
                                rows={4}
                                value={stepInputRequiredPayload}
                                onChange={(event) =>
                                  setStepInputRequiredPayload(event.target.value)
                                }
                              />
                            </>
                          )}

                          {stepAction === "resume" && (
                            <textarea
                              className="mono"
                              rows={4}
                              value={stepResumePayload}
                              onChange={(event) =>
                                setStepResumePayload(event.target.value)
                              }
                            />
                          )}

                          <button onClick={onApplyStepAction}>
                            Apply Step Action
                          </button>
                        </div>
                      </div>

                      <ul className="teams-step-list">
                        {steps.map((step) => (
                          <li key={step.id}>
                            <div className="teams-step-head">
                              <span className="mono">{step.id}</span>
                              <span>{step.step_key}</span>
                              <span>{step.status}</span>
                            </div>
                            <div className="teams-step-body mono">
                              <div>member_id: {step.member_id}</div>
                              <div>attempt: {step.attempt}</div>
                              <div>
                                depends_on: {step.depends_on.length ? step.depends_on.join(", ") : "-"}
                              </div>
                              <div>remote_task_id: {step.remote_task_id ?? "-"}</div>
                              {step.error_text && <div>error_text: {step.error_text}</div>}
                            </div>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {tab === "mailbox" && (
                    <div className="card">
                      <div className="toolbar">
                        <h3>Mailbox</h3>
                      </div>

                      {snapshot && (
                        <div className="teams-run-meta">
                          <span>
                            <strong>Pending:</strong> {snapshot.mailbox.pending}
                          </span>
                          <span>
                            <strong>Delivered:</strong> {snapshot.mailbox.delivered}
                          </span>
                          <span>
                            <strong>Dead Letter:</strong> {snapshot.mailbox.dead_letter}
                          </span>
                          <span>
                            <strong>Recent Messages:</strong>{" "}
                            {snapshot.mailbox.recent_messages.length}
                          </span>
                        </div>
                      )}

                      <div className="teams-message-grid">
                        <div className="teams-message-panel">
                          <h4>Send Message</h4>
                          <input
                            placeholder="from_actor_id"
                            value={msgFromActorId}
                            onChange={(event) => setMsgFromActorId(event.target.value)}
                          />
                          <input
                            placeholder="to_actor_id"
                            value={msgToActorId}
                            onChange={(event) => setMsgToActorId(event.target.value)}
                          />
                          <input
                            placeholder="channel (default)"
                            value={msgChannel}
                            onChange={(event) => setMsgChannel(event.target.value)}
                          />
                          <select
                            value={msgTransport}
                            onChange={(event) =>
                              setMsgTransport(event.target.value as "local" | "remote")
                            }
                          >
                            <option value="local">local</option>
                            <option value="remote">remote</option>
                          </select>
                          <textarea
                            className="mono"
                            rows={3}
                            placeholder="route JSON (required for remote)"
                            value={msgRoute}
                            onChange={(event) => setMsgRoute(event.target.value)}
                          />
                          <div className="form-row">
                            <select
                              value={msgTemplate}
                              onChange={(event) =>
                                setMsgTemplate(event.target.value as MailboxTemplateKey)
                              }
                            >
                              {MAILBOX_TEMPLATE_OPTIONS.map((option) => (
                                <option key={option.value} value={option.value}>
                                  {option.label}
                                </option>
                              ))}
                            </select>
                            <button type="button" onClick={onApplyMessageTemplate}>
                              Apply Template
                            </button>
                          </div>
                          <textarea
                            className="mono"
                            rows={4}
                            placeholder="payload JSON"
                            value={msgPayload}
                            onChange={(event) => setMsgPayload(event.target.value)}
                          />
                          <input
                            placeholder="idempotency_key (optional)"
                            value={msgIdempotencyKey}
                            onChange={(event) =>
                              setMsgIdempotencyKey(event.target.value)
                            }
                          />
                          <button onClick={onSendMessage} disabled={busy === "send-message"}>
                            Send Message
                          </button>
                        </div>

                        <div className="teams-message-panel">
                          <h4>Inbox</h4>
                          <input
                            placeholder="actor_id"
                            value={inboxActorId}
                            onChange={(event) => setInboxActorId(event.target.value)}
                          />
                          <input
                            placeholder="limit"
                            value={inboxLimit}
                            onChange={(event) => setInboxLimit(event.target.value)}
                          />
                          <input
                            placeholder="after_id (optional)"
                            value={inboxAfterId}
                            onChange={(event) => setInboxAfterId(event.target.value)}
                          />
                          <label className="checkbox">
                            <input
                              type="checkbox"
                              checked={inboxIncludeDelivered}
                              onChange={(event) =>
                                setInboxIncludeDelivered(event.target.checked)
                              }
                            />
                            include_delivered
                          </label>
                          <button
                            onClick={onRefreshInbox}
                            disabled={busy === "refresh-inbox"}
                          >
                            Refresh Inbox
                          </button>
                        </div>
                      </div>

                      <ul className="teams-message-list">
                        {snapshot?.mailbox.recent_messages.map((message) => (
                          <li key={`snapshot-${message.message_id}`}>
                            <div className="teams-message-head">
                              <span className="mono">#{message.message_id}</span>
                              <span>
                                {message.from_actor_id} → {message.to_actor_id}
                              </span>
                              <span>{message.status}</span>
                            </div>
                            <pre className="mono">{toPrettyJson(message.payload)}</pre>
                          </li>
                        ))}
                        {inbox.map((message) => (
                          <li key={message.message_id}>
                            <div className="teams-message-head">
                              <span className="mono">#{message.message_id}</span>
                              <span>
                                {message.from_actor_id} → {message.to_actor_id}
                              </span>
                              <span>{message.status}</span>
                            </div>
                            <pre className="mono">{toPrettyJson(message.payload)}</pre>
                            <div className="actions">
                              <button
                                onClick={() => void onAckMessage(message)}
                                disabled={
                                  message.status === "delivered" ||
                                  busy === `ack-${message.message_id}`
                                }
                              >
                                Ack
                              </button>
                            </div>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {tab === "member_console" && (
                    <div className="card">
                      <div className="toolbar">
                        <h3>Member Console</h3>
                        <div className="actions">
                          <button
                            onClick={() => {
                              if (selectedMemberSnapshot) {
                                void loadMemberEvents("replace");
                                return;
                              }
                              if (activeRunId) {
                                void refreshEvents(activeRunId);
                              }
                            }}
                            disabled={selectedMemberSnapshot ? memberEventsLoading : eventsLoading}
                          >
                            Refresh
                          </button>
                          <button
                            onClick={() => {
                              if (!selectedMemberSnapshot) return;
                              void loadMemberEvents("prepend");
                            }}
                            disabled={
                              !selectedMemberSnapshot ||
                              memberEventsLoading ||
                              !memberEventsHasMore ||
                              oldestMemberEventId == null
                            }
                          >
                            Load Older
                          </button>
                        </div>
                      </div>

                      <div className="form-row">
                        <select
                          value={selectedMemberId}
                          onChange={(event) => setSelectedMemberId(event.target.value)}
                        >
                          <option value="">Select member</option>
                          {snapshot?.members.map((member) => (
                            <option key={member.member_id} value={member.member_id}>
                              {member.member_id} ({member.role})
                            </option>
                          ))}
                        </select>
                      </div>

                      {selectedMemberSnapshot && (
                        <div className="teams-step-body mono">
                          <div>member_id: {selectedMemberSnapshot.member_id}</div>
                          <div>role: {selectedMemberSnapshot.role}</div>
                          <div>model: {selectedMemberSnapshot.model ?? "-"}</div>
                          <div>status: {selectedMemberSnapshot.status}</div>
                          <div>
                            session_status: {selectedMemberSnapshot.session_status ?? "-"}
                          </div>
                          <div>
                            remote_task_id:{" "}
                            {selectedMemberSnapshot.latest_step?.remote_task_id ?? "-"}
                          </div>
                          <div>
                            skills:{" "}
                            {selectedMemberSnapshot.skills.length > 0
                              ? selectedMemberSnapshot.skills.join(", ")
                              : "-"}
                          </div>
                          <div>prompt: {selectedMemberSnapshot.prompt ?? "-"}</div>
                        </div>
                      )}

                      {!selectedMemberSnapshot && (
                        <p className="muted">
                          Showing latest {TEAM_EVENT_PREVIEW_LIMIT} run records. Select a member
                          for full member history.
                        </p>
                      )}

                      {selectedMemberSnapshot &&
                        !selectedMemberSnapshot.latest_step?.remote_task_id && (
                          <p className="muted">
                            Selected member has no associated session yet.
                          </p>
                        )}

                      {selectedMemberSnapshot &&
                        selectedMemberSnapshot.latest_step?.remote_task_id &&
                        memberEvents.length === 0 && (
                          <p className="muted">No member events yet.</p>
                        )}

                      {!selectedMemberSnapshot && displayedRunEvents.length === 0 && (
                        <p className="muted">No run records yet.</p>
                      )}

                      {selectedMemberSnapshot && (
                        <ul className="teams-event-list">
                          {memberEvents.map((event) => (
                            <li key={event.event_id}>
                              <div className="teams-event-head">
                                <span className="mono">#{event.event_id}</span>
                                <span>{event.stream}</span>
                                <span>{formatTs(event.ts)}</span>
                              </div>
                              <pre className="mono">{event.message}</pre>
                            </li>
                          ))}
                        </ul>
                      )}

                      {!selectedMemberSnapshot && (
                        <ul className="teams-event-list">
                          {displayedRunEvents.map((event) => (
                            <li key={event.event_id}>
                              <div className="teams-event-head">
                                <span className="mono">#{event.event_id}</span>
                                <span>{event.event_type}</span>
                                <span>{formatTs(event.ts)}</span>
                              </div>
                              <pre className="mono">{toPrettyJson(event.payload)}</pre>
                            </li>
                          ))}
                        </ul>
                      )}
                    </div>
                  )}
                </>
              )}
            </>
          )}
        </div>
      </section>

      {showCreateTeamModal && (
        <div
          className="modal-backdrop team-create-modal-backdrop"
          role="presentation"
          onClick={(event) => {
            if (event.target === event.currentTarget && busy !== "create-team") {
              closeCreateTeamModal();
            }
          }}
        >
          <div
            className="modal team-create-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="team-create-title"
          >
            <div className="modal-head">
              <h3 id="team-create-title">Team Forge</h3>
              <span className="badge">
                Stage {createTeamStage + 1}/{CREATE_TEAM_STAGE_TITLES.length}
              </span>
            </div>

            <div className="team-create-progress">
              {CREATE_TEAM_STAGE_TITLES.map((title, index) => {
                const stageIndex = index as CreateTeamStage;
                const isActive = stageIndex === createTeamStage;
                const isCompleted = stageIndex < createTeamStage;
                const isLocked = !canEnterCreateStage(stageIndex);
                return (
                  <button
                    key={title}
                    className={
                      isActive
                        ? "team-create-stage active"
                        : isCompleted
                          ? "team-create-stage completed"
                          : isLocked
                            ? "team-create-stage locked"
                          : "team-create-stage"
                    }
                    onClick={() => onSelectCreateTeamStage(stageIndex)}
                    type="button"
                    aria-disabled={isLocked && !isActive && !isCompleted}
                    title={
                      isLocked && !isActive && !isCompleted
                        ? "Complete previous stage requirements first"
                        : undefined
                    }
                  >
                    <span className="team-create-stage-index">#{index + 1}</span>
                    <span className="team-create-stage-title">{title}</span>
                  </button>
                );
              })}
            </div>

            <div className="modal-body">
              <div className="team-create-checklist">
                {questChecklist.map((item) => (
                  <div
                    key={item.key}
                    className={
                      item.ready
                        ? "team-create-check-item ready"
                        : "team-create-check-item pending"
                    }
                  >
                    <span
                      className="team-create-check-icon"
                      aria-hidden="true"
                    >
                      {item.ready ? "✓" : "○"}
                    </span>
                    <span>{item.label}</span>
                  </div>
                ))}
              </div>

              <div className="team-create-agent-entry">
                <div className="team-create-agent-entry-head">
                  <h4>Agent Forge</h4>
                  <button
                    onClick={showForgeAgentForm ? closeForgeAgentForm : openForgeAgentForm}
                    disabled={useSpecOverride || forgeAgentBusy}
                    type="button"
                  >
                    {showForgeAgentForm ? "Hide" : "New Agent"}
                  </button>
                </div>
                <p className="muted">
                  Use one unified entry to create an agent, then optionally bind it to leader or
                  workers.
                </p>
                {showForgeAgentForm && (
                  <div className="team-create-forge-agent">
                    <div className="team-create-forge-agent-head">
                      <strong>Create Agent</strong>
                      <button
                        onClick={closeForgeAgentForm}
                        disabled={forgeAgentBusy}
                        type="button"
                      >
                        Close
                      </button>
                    </div>
                    <div className="team-create-forge-agent-grid">
                      <input
                        placeholder="agent name"
                        value={forgeAgentName}
                        onChange={(event) => setForgeAgentName(event.target.value)}
                        disabled={forgeAgentBusy}
                      />
                      <input
                        placeholder="workdir"
                        value={forgeAgentWorkdir}
                        onChange={(event) => setForgeAgentWorkdir(event.target.value)}
                        disabled={forgeAgentBusy}
                      />
                      <select
                        value={forgeAgentPresetId}
                        onChange={(event) => {
                          const next = event.target.value;
                          if (isAgentPresetId(next)) {
                            setForgeAgentPresetId(next);
                          }
                        }}
                        disabled={forgeAgentBusy}
                      >
                        {forgeAgentPresetOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                      <select
                        value={forgeAgentBindTarget}
                        onChange={(event) =>
                          setForgeAgentBindTarget(event.target.value as TeamForgeBindTarget)
                        }
                        disabled={forgeAgentBusy}
                      >
                        <option value="none">Create only (no binding)</option>
                        <option value="leader">Create and bind as leader</option>
                        <option value="worker">Create and bind as worker</option>
                      </select>
                      <label className="checkbox">
                        <input
                          type="checkbox"
                          checked={forgeAgentCodeMode}
                          onChange={(event) => setForgeAgentCodeMode(event.target.checked)}
                          disabled={forgeAgentBusy}
                        />
                        code_mode
                      </label>
                    </div>
                    <div className="team-create-forge-agent-meta mono">
                      <span>{`command=${forgeAgentPreset.command}`}</span>
                      <span>{`args=${
                        forgeAgentPreset.args.length > 0
                          ? forgeAgentPreset.args.join(" ")
                          : "-"
                      }`}</span>
                      <span>{`bind_target=${forgeAgentBindTarget}`}</span>
                    </div>
                    <div className="team-create-forge-agent-actions">
                      <button
                        onClick={onCreateForgeAgent}
                        disabled={forgeAgentBusy}
                        type="button"
                      >
                        {forgeAgentBusy ? "Creating..." : "Create Agent"}
                      </button>
                    </div>
                  </div>
                )}
              </div>

              {createTeamStage === 0 && (
                <div className="team-create-panel">
                  <h4>Mission Brief</h4>
                  <p className="muted">
                    Pick a team name and description first. This is the party identity shown in
                    the workbench.
                  </p>
                  {!isMissionBriefReady && (
                    <p className="team-create-stage-note">
                      Team name is required before entering the next stage.
                    </p>
                  )}
                  <input
                    placeholder="team name"
                    value={newTeamName}
                    onChange={(event) => setNewTeamName(event.target.value)}
                  />
                  <input
                    placeholder="description (optional)"
                    value={newTeamDescription}
                    onChange={(event) => setNewTeamDescription(event.target.value)}
                  />
                </div>
              )}

              {createTeamStage === 1 && (
                <div className="team-create-panel">
                  <h4>Leader Forge</h4>
                  <p className="muted">
                    Choose the leader agent first. Its existing workdir/worktree config will be
                    reused when this team run starts.
                  </p>
                  {!isLeaderForgeReady && hasAgents && (
                    <p className="team-create-stage-note">
                      Select one leader agent to continue.
                    </p>
                  )}
                  {!hasAgents && (
                    <p className="muted">
                      No agents available yet. Create one in the Agent Forge entry above, or in
                      `Agents` mode first.
                    </p>
                  )}
                  <select
                    value={leaderMemberId}
                    onChange={(event) => setLeaderMemberId(event.target.value)}
                    disabled={useSpecOverride || !hasAgents}
                  >
                    <option value="">Select leader agent</option>
                    {leaderAgentSelectOptions.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                  <div className="teams-step-body mono">
                    <div>agent_id: {leaderMemberId || "-"}</div>
                    <div>workdir: {leaderAgent?.workdir ?? "-"}</div>
                  </div>
                  <select
                    value={leaderModel}
                    onChange={(event) => setLeaderModel(event.target.value)}
                    disabled={useSpecOverride}
                  >
                    {leaderModelOptions.map((option) => (
                      <option key={option.value || "__default"} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                  <div className="team-skill-tags">
                    {TEAM_SKILL_OPTIONS.map((skill) => {
                      const selected = leaderSkills.includes(skill);
                      return (
                        <button
                          key={`leader-skill-${skill}`}
                          type="button"
                          className={selected ? "team-skill-tag selected" : "team-skill-tag"}
                          onClick={() =>
                            setLeaderSkills((prev) => toggleSkillSelection(prev, skill))
                          }
                          disabled={useSpecOverride}
                        >
                          {skill}
                        </button>
                      );
                    })}
                  </div>
                  <input
                    placeholder="leader custom skills (comma separated, optional)"
                    value={leaderCustomSkills}
                    onChange={(event) => setLeaderCustomSkills(event.target.value)}
                    disabled={useSpecOverride}
                  />
                  <textarea
                    className="mono"
                    rows={4}
                    placeholder="leader prompt"
                    value={leaderPrompt}
                    onChange={(event) => setLeaderPrompt(event.target.value)}
                    disabled={useSpecOverride}
                  />
                </div>
              )}

              {createTeamStage === 2 && (
                <div className="team-create-panel">
                  <div className="toolbar">
                    <h4>Recruit Workers</h4>
                    <div className="toolbar-actions">
                      <button onClick={onAddWorker} disabled={useSpecOverride || !hasAgents}>
                        Add Worker
                      </button>
                      <button
                        onClick={onAddAllRemainingWorkers}
                        disabled={useSpecOverride || !hasAgents || availableWorkerAgentCount === 0}
                        type="button"
                      >
                        Auto Fill Party
                      </button>
                    </div>
                  </div>
                  <p className="muted">
                    Build your party. Each worker maps to an existing agent (and reuses its
                    workdir/worktree config). Worker model/prompt/skills can still be customized
                    at team level.
                  </p>
                  {unassignedWorkerSlots > 0 && (
                    <p className="team-create-stage-note">
                      {unassignedWorkerSlots} worker slot
                      {unassignedWorkerSlots > 1 ? "s are" : " is"} currently unassigned and will
                      be ignored unless selected.
                    </p>
                  )}
                  <div className="team-create-worker-grid">
                    {workers.map((worker, index) => {
                      const selectedByOthers = new Set(
                        workers
                          .filter((_, workerIndex) => workerIndex !== index)
                          .map((item) => item.member_id.trim())
                          .filter((item) => item.length > 0)
                      );
                      const workerOptions = leaderAgentOptions.filter((option) => {
                        if (option.value === worker.member_id) return true;
                        if (option.value === leaderMemberId.trim()) return false;
                        return !selectedByOthers.has(option.value);
                      });
                      const hasSelectedWorker = workerOptions.some(
                        (option) => option.value === worker.member_id
                      );
                      if (worker.member_id && !hasSelectedWorker) {
                        workerOptions.unshift({
                          value: worker.member_id,
                          label: `Missing agent (${worker.member_id})`,
                        });
                      }
                      const workerAgent = agents.find(
                        (agent) => agent.id === worker.member_id
                      );
                      return (
                        <div key={`worker-${index}`} className="teams-worker-card">
                          <div className="team-create-worker-head">
                            <strong>Worker {index + 1}</strong>
                            <button
                              onClick={() => onRemoveWorker(index)}
                              disabled={useSpecOverride}
                              type="button"
                            >
                              Remove
                            </button>
                          </div>
                          <select
                            value={worker.member_id}
                            onChange={(event) =>
                              onUpdateWorker(index, "member_id", event.target.value)
                            }
                            disabled={useSpecOverride || !hasAgents}
                          >
                            <option value="">Select worker agent</option>
                            {workerOptions.map((option) => (
                              <option key={option.value} value={option.value}>
                                {option.label}
                              </option>
                            ))}
                          </select>
                          <div className="teams-step-body mono">
                            <div>agent_id: {worker.member_id || "-"}</div>
                            <div>workdir: {workerAgent?.workdir ?? "-"}</div>
                          </div>
                          <select
                            value={worker.model}
                            onChange={(event) =>
                              onUpdateWorker(index, "model", event.target.value)
                            }
                            disabled={useSpecOverride}
                          >
                            {resolveTeamModelOptions(worker.model).map((option) => (
                              <option key={option.value || "__default"} value={option.value}>
                                {option.label}
                              </option>
                            ))}
                          </select>
                          <div className="team-skill-tags">
                            {TEAM_SKILL_OPTIONS.map((skill) => {
                              const selected = worker.skills.includes(skill);
                              return (
                                <button
                                  key={`worker-skill-${index}-${skill}`}
                                  type="button"
                                  className={
                                    selected ? "team-skill-tag selected" : "team-skill-tag"
                                  }
                                  onClick={() => onToggleWorkerSkill(index, skill)}
                                  disabled={useSpecOverride}
                                >
                                  {skill}
                                </button>
                              );
                            })}
                          </div>
                          <input
                            placeholder="worker custom skills (comma separated, optional)"
                            value={worker.custom_skills}
                            onChange={(event) =>
                              onUpdateWorker(index, "custom_skills", event.target.value)
                            }
                            disabled={useSpecOverride}
                          />
                          <textarea
                            className="mono"
                            rows={3}
                            placeholder="worker prompt"
                            value={worker.prompt}
                            onChange={(event) =>
                              onUpdateWorker(index, "prompt", event.target.value)
                            }
                            disabled={useSpecOverride}
                          />
                        </div>
                      );
                    })}
                  </div>
                  {workers.length === 0 && (
                    <p className="muted">No workers configured. Team will run with leader only.</p>
                  )}
                  {hasDuplicateMembers && (
                    <div className="team-create-warning">
                      <p className="muted">
                        Duplicate assignments detected: {duplicateMemberIds.join(", ")}. Leader
                        and workers must reference different agents.
                      </p>
                      <button onClick={onResolveDuplicateWorkers} type="button">
                        Resolve Duplicates
                      </button>
                    </div>
                  )}
                </div>
              )}

              {createTeamStage === 3 && (
                <div className="team-create-panel">
                  <h4>Launch Team</h4>
                  <p className="muted">
                    Final review before deployment. You can still override spec JSON manually.
                  </p>
                  <div className="teams-run-meta mono">
                    <span>team={newTeamName.trim() || "-"}</span>
                    <span>leader={leaderMemberId.trim() || "-"}</span>
                    <span>workers={configuredWorkerCount}</span>
                  </div>
                  <p className="muted">
                    Default workflow is generated automatically:
                    `leader_plan` → `worker_*` → `leader_synthesize`.
                  </p>
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={useSpecOverride}
                      onChange={(event) => setUseSpecOverride(event.target.checked)}
                    />
                    Edit spec JSON manually
                  </label>
                  <textarea
                    className="mono"
                    rows={12}
                    value={displayedTeamSpec}
                    onChange={(event) => setNewTeamSpec(event.target.value)}
                    readOnly={!useSpecOverride}
                  />
                </div>
              )}
            </div>

            <div className="modal-actions team-create-actions">
              {!canAdvanceCreateStage && currentStageBlockReason && (
                <span className="team-create-actions-note">{currentStageBlockReason}</span>
              )}
              <button
                className="ghost"
                onClick={closeCreateTeamModal}
                disabled={busy === "create-team"}
                type="button"
              >
                Cancel
              </button>
              <button
                className="ghost"
                onClick={goToPrevCreateTeamStage}
                disabled={createTeamStage === 0 || busy === "create-team"}
                type="button"
              >
                Back
              </button>
              {createTeamStage < 3 && (
                <button
                  onClick={goToNextCreateTeamStage}
                  disabled={!canAdvanceCreateStage || busy === "create-team"}
                  type="button"
                >
                  Next Stage
                </button>
              )}
              {createTeamStage === 3 && (
                <button
                  onClick={onCreateTeam}
                  disabled={
                    busy === "create-team" ||
                    (!useSpecOverride &&
                      (!hasAgents || !leaderMemberId.trim() || hasDuplicateMembers))
                  }
                  type="button"
                >
                  Create Team
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
