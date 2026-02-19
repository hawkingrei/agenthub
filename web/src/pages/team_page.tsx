import React, { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
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
  listAgentPresets,
  type AgentPresetId,
} from "../agent_presets";
import { isAgentActiveStatus } from "../agent_ws";
import {
  StatusBadge,
  resolveTeamRunStatusTone,
} from "../components/status_badge";
import { CreateAgentModal } from "../components/create_agent_modal";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";
import {
  normalizeRuntimeWorktreeRoot,
  normalizeWorkdirInput,
  resolveWorkdirForModeChange,
  resolveWorkdirForModalOpen,
} from "../worktree_defaults";
import { TeamEventsPanel } from "./team_events_panel";
import { TeamMailboxPanel } from "./team_mailbox_panel";
import { TeamMemberConsolePanel } from "./team_member_console_panel";
import { TeamOverviewPanel } from "./team_overview_panel";
import { TeamRunPanel } from "./team_run_panel";
import { TeamSidebar } from "./team_sidebar";
import { TeamStepsPanel } from "./team_steps_panel";

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
type TeamUiState = {
  tab: TeamTab;
  runLookupId: string;
  eventsAutoRefresh: boolean;
};
type TeamUiAction =
  | { type: "set_tab"; tab: TeamTab }
  | { type: "set_run_lookup_id"; runLookupId: string }
  | { type: "set_events_auto_refresh"; eventsAutoRefresh: boolean };
type TeamControlState = {
  runContextId: string;
  runInput: string;
  stepKey: string;
  stepMemberId: string;
  stepDependsOn: string;
  stepInput: string;
  selectedStepId: string;
  stepAction: StepAction;
  stepRemoteTaskId: string;
  stepOutput: string;
  stepFailText: string;
  stepInputReason: string;
  stepInputRequiredPayload: string;
  stepResumePayload: string;
};
type TeamControlAction = { type: "patch"; patch: Partial<TeamControlState> };
type TeamMailboxState = {
  msgFromActorId: string;
  msgToActorId: string;
  msgChannel: string;
  msgTransport: "local" | "remote";
  msgRoute: string;
  msgTemplate: MailboxTemplateKey;
  msgPayload: string;
  msgIdempotencyKey: string;
  chatDraft: string;
  chatStickToBottom: boolean;
  chatSeenByConversation: Record<string, number>;
  inboxActorId: string;
  inboxLimit: string;
  inboxAfterId: string;
  inboxIncludeDelivered: boolean;
  inbox: TeamActorMessageRecord[];
  selectedMemberId: string;
};
type TeamMailboxAction =
  | { type: "patch"; patch: Partial<TeamMailboxState> }
  | { type: "mark_conversation_seen"; key: string; messageId: number }
  | { type: "reset_chat_seen" };
type TeamCreateState = TeamCreateDraftState & {
  newTeamName: string;
  newTeamDescription: string;
  showCreateTeamModal: boolean;
  createTeamStage: CreateTeamStage;
  forgeAgentBindTarget: TeamForgeBindTarget;
  showForgeAgentForm: boolean;
  forgeAgentName: string;
  forgeAgentWorkdir: string;
  forgeAgentPresetId: AgentPresetId;
  forgeAgentWorktreeMode: "use_existing" | "create_worktree" | "reuse_worktree";
  forgeAgentWorktreeRepo: string;
  forgeAgentWorktreeRef: string;
  forgeAgentCodeMode: boolean;
  forgeAgentWorktreeError: string | null;
  forgeAgentBusy: boolean;
};
type TeamCreateAction = { type: "patch"; patch: Partial<TeamCreateState> };

const EVENT_PAGE_LIMIT = 100;
const MEMBER_EVENT_PAGE_LIMIT = 300;
const TEAM_RUN_PAGE_LIMIT = 50;
const TEAM_EVENT_PREVIEW_LIMIT = 5;
const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";
const DEFAULT_TEAM_RUN_BROWSER_STATE: TeamRunBrowserState = {
  statusFilter: "all",
  hasMore: false,
};
const DEFAULT_TEAM_UI_STATE: TeamUiState = {
  tab: "overview",
  runLookupId: "",
  eventsAutoRefresh: true,
};
const DEFAULT_TEAM_CONTROL_STATE: TeamControlState = {
  runContextId: "",
  runInput: "{}",
  stepKey: "",
  stepMemberId: "",
  stepDependsOn: "",
  stepInput: "{}",
  selectedStepId: "",
  stepAction: "start",
  stepRemoteTaskId: "",
  stepOutput: "{}",
  stepFailText: "",
  stepInputReason: "",
  stepInputRequiredPayload: "{}",
  stepResumePayload: "{}",
};
const DEFAULT_TEAM_MAILBOX_STATE: TeamMailboxState = {
  msgFromActorId: "",
  msgToActorId: "",
  msgChannel: "default",
  msgTransport: "local",
  msgRoute: "",
  msgTemplate: "leader_task_assignment",
  msgPayload: "{}",
  msgIdempotencyKey: "",
  chatDraft: "",
  chatStickToBottom: true,
  chatSeenByConversation: {},
  inboxActorId: "",
  inboxLimit: "100",
  inboxAfterId: "",
  inboxIncludeDelivered: false,
  inbox: [],
  selectedMemberId: "",
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

function reduceTeamUiState(state: TeamUiState, action: TeamUiAction): TeamUiState {
  switch (action.type) {
    case "set_tab":
      return { ...state, tab: action.tab };
    case "set_run_lookup_id":
      return { ...state, runLookupId: action.runLookupId };
    case "set_events_auto_refresh":
      return { ...state, eventsAutoRefresh: action.eventsAutoRefresh };
    default:
      return state;
  }
}

function reduceTeamControlState(
  state: TeamControlState,
  action: TeamControlAction
): TeamControlState {
  switch (action.type) {
    case "patch":
      return { ...state, ...action.patch };
    default:
      return state;
  }
}

function reduceTeamMailboxState(
  state: TeamMailboxState,
  action: TeamMailboxAction
): TeamMailboxState {
  switch (action.type) {
    case "patch":
      return { ...state, ...action.patch };
    case "mark_conversation_seen": {
      if (!action.key) {
        return state;
      }
      const current = state.chatSeenByConversation[action.key] ?? 0;
      if (action.messageId <= current) {
        return state;
      }
      return {
        ...state,
        chatSeenByConversation: {
          ...state.chatSeenByConversation,
          [action.key]: action.messageId,
        },
      };
    }
    case "reset_chat_seen":
      if (Object.keys(state.chatSeenByConversation).length === 0) {
        return state;
      }
      return { ...state, chatSeenByConversation: {} };
    default:
      return state;
  }
}

function reduceTeamCreateState(
  state: TeamCreateState,
  action: TeamCreateAction
): TeamCreateState {
  switch (action.type) {
    case "patch":
      return { ...state, ...action.patch };
    default:
      return state;
  }
}

function resolveUpdater<T>(current: T, next: T | ((prev: T) => T)): T {
  if (typeof next === "function") {
    return (next as (prev: T) => T)(current);
  }
  return next;
}

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
const DEFAULT_TEAM_LEADER_SKILLS = [
  "agenthub-actor-runtime",
  "team-leader-orchestrator",
  "team-deliberation-rules",
];
const DEFAULT_TEAM_WORKER_SKILLS = [
  "agenthub-actor-runtime",
  "team-worker-executor",
  "team-deliberation-rules",
];
const REQUIRED_TEAM_LEADER_SKILLS = [
  "agenthub-actor-runtime",
  "team-leader-orchestrator",
];
const REQUIRED_TEAM_WORKER_SKILLS = [
  "agenthub-actor-runtime",
  "team-worker-executor",
];
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

export type TeamCreateDraftState = {
  leaderMemberId: string;
  leaderModel: string;
  leaderPrompt: string;
  leaderSkills: string[];
  leaderCustomSkills: string;
  workers: Array<{
    member_id: string;
    model: string;
    prompt: string;
    skills: string[];
    custom_skills: string;
  }>;
  useSpecOverride: boolean;
  newTeamSpec: string;
  teamForgeAgentIds: string[];
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

export type TeamMailboxChatActors = {
  fromActorId: string;
  toActorId: string;
  inboxActorId: string;
};

export function resolveMailboxChatActors(
  leaderMemberId: string | null | undefined,
  memberIds: string[],
  selectedMemberId: string
): TeamMailboxChatActors {
  if (memberIds.length === 0) {
    return {
      fromActorId: "",
      toActorId: "",
      inboxActorId: "",
    };
  }
  const normalizedLeaderId = (leaderMemberId ?? "").trim();
  const leaderId = normalizedLeaderId && memberIds.includes(normalizedLeaderId)
    ? normalizedLeaderId
    : memberIds[0] ?? "";
  const normalizedSelectedId = selectedMemberId.trim();
  const targetId = normalizedSelectedId && memberIds.includes(normalizedSelectedId)
    ? normalizedSelectedId
    : memberIds[0] ?? "";
  return {
    fromActorId: leaderId,
    toActorId: targetId,
    inboxActorId: targetId,
  };
}

export function mergeMailboxMessages(
  recentMessages: TeamActorMessageRecord[],
  inboxMessages: TeamActorMessageRecord[]
): TeamActorMessageRecord[] {
  const byId = new Map<number, TeamActorMessageRecord>();
  for (const message of [...recentMessages, ...inboxMessages]) {
    byId.set(message.message_id, message);
  }
  return [...byId.values()].sort((a, b) => a.message_id - b.message_id);
}

export function selectMailboxConversation(
  messages: TeamActorMessageRecord[],
  actorA: string,
  actorB: string
): TeamActorMessageRecord[] {
  const left = actorA.trim();
  const right = actorB.trim();
  if (!left || !right) {
    return [];
  }
  return messages.filter(
    (message) =>
      (message.from_actor_id === left && message.to_actor_id === right) ||
      (message.from_actor_id === right && message.to_actor_id === left)
  );
}

export function buildMailboxChatPayload(text: string): {
  type: "chat_message";
  text: string;
  source: "team_workbench";
} {
  return {
    type: "chat_message",
    text,
    source: "team_workbench",
  };
}

export function buildMailboxConversationKey(actorA: string, actorB: string): string {
  const pair = [actorA.trim(), actorB.trim()].filter((value) => value.length > 0).sort();
  if (pair.length < 2) {
    return "";
  }
  return `${pair[0]}::${pair[1]}`;
}

export function resolveConversationMaxMessageId(
  messages: TeamActorMessageRecord[]
): number | null {
  if (messages.length === 0) {
    return null;
  }
  return messages.reduce(
    (maxId, message) => (message.message_id > maxId ? message.message_id : maxId),
    messages[0]?.message_id ?? 0
  );
}

export function countUnreadConversationMessages(
  messages: TeamActorMessageRecord[],
  actorA: string,
  actorB: string,
  seenMessageId: number
): number {
  const left = actorA.trim();
  const right = actorB.trim();
  if (!left || !right) {
    return 0;
  }
  return messages.filter((message) => {
    if (message.message_id <= seenMessageId) {
      return false;
    }
    const inConversation =
      (message.from_actor_id === left && message.to_actor_id === right) ||
      (message.from_actor_id === right && message.to_actor_id === left);
    if (!inConversation) {
      return false;
    }
    // Unread in chat list should reflect inbound messages to the current sender side.
    if (left === right) {
      return true;
    }
    return message.to_actor_id === left;
  }).length;
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

export function createInitialTeamDraftState(): TeamCreateDraftState {
  return {
    leaderMemberId: "",
    leaderModel: "",
    leaderPrompt: DEFAULT_TEAM_LEADER_PROMPT,
    leaderSkills: [...DEFAULT_TEAM_LEADER_SKILLS],
    leaderCustomSkills: "",
    workers: [],
    useSpecOverride: false,
    newTeamSpec: "{}",
    teamForgeAgentIds: [],
  };
}

function createInitialTeamCreateState(): TeamCreateState {
  const draft = createInitialTeamDraftState();
  return {
    ...draft,
    newTeamName: "",
    newTeamDescription: "",
    showCreateTeamModal: false,
    createTeamStage: 0,
    forgeAgentBindTarget: "none",
    showForgeAgentForm: false,
    forgeAgentName: "",
    forgeAgentWorkdir: "",
    forgeAgentPresetId: DEFAULT_AGENT_PRESET_ID,
    forgeAgentWorktreeMode: "use_existing",
    forgeAgentWorktreeRepo: "",
    forgeAgentWorktreeRef: "",
    forgeAgentCodeMode: true,
    forgeAgentWorktreeError: null,
    forgeAgentBusy: false,
  };
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

function formatTeamForgeWorktreeError(err: unknown): string | null {
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

  const [teamUiState, dispatchTeamUi] = useReducer(
    reduceTeamUiState,
    DEFAULT_TEAM_UI_STATE
  );
  const tab = teamUiState.tab;
  const runLookupId = teamUiState.runLookupId;
  const eventsAutoRefresh = teamUiState.eventsAutoRefresh;
  const setTab = useCallback((next: TeamTab) => {
    dispatchTeamUi({ type: "set_tab", tab: next });
  }, []);
  const setRunLookupId = useCallback((next: string) => {
    dispatchTeamUi({ type: "set_run_lookup_id", runLookupId: next });
  }, []);
  const setEventsAutoRefresh = useCallback((next: boolean) => {
    dispatchTeamUi({ type: "set_events_auto_refresh", eventsAutoRefresh: next });
  }, []);
  const [teamControlState, dispatchTeamControl] = useReducer(
    reduceTeamControlState,
    DEFAULT_TEAM_CONTROL_STATE
  );
  const runContextId = teamControlState.runContextId;
  const runInput = teamControlState.runInput;
  const stepKey = teamControlState.stepKey;
  const stepMemberId = teamControlState.stepMemberId;
  const stepDependsOn = teamControlState.stepDependsOn;
  const stepInput = teamControlState.stepInput;
  const selectedStepId = teamControlState.selectedStepId;
  const stepAction = teamControlState.stepAction;
  const stepRemoteTaskId = teamControlState.stepRemoteTaskId;
  const stepOutput = teamControlState.stepOutput;
  const stepFailText = teamControlState.stepFailText;
  const stepInputReason = teamControlState.stepInputReason;
  const stepInputRequiredPayload = teamControlState.stepInputRequiredPayload;
  const stepResumePayload = teamControlState.stepResumePayload;
  const patchTeamControl = useCallback((patch: Partial<TeamControlState>) => {
    dispatchTeamControl({ type: "patch", patch });
  }, []);
  const setRunContextId = useCallback(
    (next: string) => patchTeamControl({ runContextId: next }),
    [patchTeamControl]
  );
  const setRunInput = useCallback(
    (next: string) => patchTeamControl({ runInput: next }),
    [patchTeamControl]
  );
  const setStepKey = useCallback(
    (next: string) => patchTeamControl({ stepKey: next }),
    [patchTeamControl]
  );
  const setStepMemberId = useCallback(
    (next: string) => patchTeamControl({ stepMemberId: next }),
    [patchTeamControl]
  );
  const setStepDependsOn = useCallback(
    (next: string) => patchTeamControl({ stepDependsOn: next }),
    [patchTeamControl]
  );
  const setStepInput = useCallback(
    (next: string) => patchTeamControl({ stepInput: next }),
    [patchTeamControl]
  );
  const setSelectedStepId = useCallback(
    (next: string) => patchTeamControl({ selectedStepId: next }),
    [patchTeamControl]
  );
  const setStepAction = useCallback(
    (next: StepAction) => patchTeamControl({ stepAction: next }),
    [patchTeamControl]
  );
  const setStepRemoteTaskId = useCallback(
    (next: string) => patchTeamControl({ stepRemoteTaskId: next }),
    [patchTeamControl]
  );
  const setStepOutput = useCallback(
    (next: string) => patchTeamControl({ stepOutput: next }),
    [patchTeamControl]
  );
  const setStepFailText = useCallback(
    (next: string) => patchTeamControl({ stepFailText: next }),
    [patchTeamControl]
  );
  const setStepInputReason = useCallback(
    (next: string) => patchTeamControl({ stepInputReason: next }),
    [patchTeamControl]
  );
  const setStepInputRequiredPayload = useCallback(
    (next: string) => patchTeamControl({ stepInputRequiredPayload: next }),
    [patchTeamControl]
  );
  const setStepResumePayload = useCallback(
    (next: string) => patchTeamControl({ stepResumePayload: next }),
    [patchTeamControl]
  );

  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [teams, setTeams] = useState<TeamDefinitionRecord[]>([]);
  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(null);

  const [teamCreateState, dispatchTeamCreate] = useReducer(
    reduceTeamCreateState,
    undefined,
    createInitialTeamCreateState
  );
  const newTeamName = teamCreateState.newTeamName;
  const newTeamDescription = teamCreateState.newTeamDescription;
  const useSpecOverride = teamCreateState.useSpecOverride;
  const newTeamSpec = teamCreateState.newTeamSpec;
  const showCreateTeamModal = teamCreateState.showCreateTeamModal;
  const createTeamStage = teamCreateState.createTeamStage;
  const leaderMemberId = teamCreateState.leaderMemberId;
  const leaderModel = teamCreateState.leaderModel;
  const leaderPrompt = teamCreateState.leaderPrompt;
  const leaderSkills = teamCreateState.leaderSkills;
  const leaderCustomSkills = teamCreateState.leaderCustomSkills;
  const workers = teamCreateState.workers;
  const forgeAgentBindTarget = teamCreateState.forgeAgentBindTarget;
  const showForgeAgentForm = teamCreateState.showForgeAgentForm;
  const forgeAgentName = teamCreateState.forgeAgentName;
  const forgeAgentWorkdir = teamCreateState.forgeAgentWorkdir;
  const forgeAgentPresetId = teamCreateState.forgeAgentPresetId;
  const forgeAgentWorktreeMode = teamCreateState.forgeAgentWorktreeMode;
  const forgeAgentWorktreeRepo = teamCreateState.forgeAgentWorktreeRepo;
  const forgeAgentWorktreeRef = teamCreateState.forgeAgentWorktreeRef;
  const forgeAgentCodeMode = teamCreateState.forgeAgentCodeMode;
  const forgeAgentWorktreeError = teamCreateState.forgeAgentWorktreeError;
  const forgeAgentBusy = teamCreateState.forgeAgentBusy;
  const teamForgeAgentIds = teamCreateState.teamForgeAgentIds;
  const [forgeDefaultWorktreeRoot, setForgeDefaultWorktreeRoot] = useState(
    DEFAULT_WORKTREE_ROOT
  );
  const patchTeamCreate = useCallback((patch: Partial<TeamCreateState>) => {
    dispatchTeamCreate({ type: "patch", patch });
  }, []);
  const setNewTeamName = useCallback(
    (next: string) => patchTeamCreate({ newTeamName: next }),
    [patchTeamCreate]
  );
  const setNewTeamDescription = useCallback(
    (next: string) => patchTeamCreate({ newTeamDescription: next }),
    [patchTeamCreate]
  );
  const setUseSpecOverride = useCallback(
    (next: boolean) => patchTeamCreate({ useSpecOverride: next }),
    [patchTeamCreate]
  );
  const setNewTeamSpec = useCallback(
    (next: string) => patchTeamCreate({ newTeamSpec: next }),
    [patchTeamCreate]
  );
  const setShowCreateTeamModal = useCallback(
    (next: boolean) => patchTeamCreate({ showCreateTeamModal: next }),
    [patchTeamCreate]
  );
  const setCreateTeamStage = useCallback(
    (next: CreateTeamStage | ((prev: CreateTeamStage) => CreateTeamStage)) =>
      patchTeamCreate({ createTeamStage: resolveUpdater(createTeamStage, next) }),
    [createTeamStage, patchTeamCreate]
  );
  const setLeaderMemberId = useCallback(
    (next: string) => patchTeamCreate({ leaderMemberId: next }),
    [patchTeamCreate]
  );
  const setLeaderModel = useCallback(
    (next: string) => patchTeamCreate({ leaderModel: next }),
    [patchTeamCreate]
  );
  const setLeaderPrompt = useCallback(
    (next: string) => patchTeamCreate({ leaderPrompt: next }),
    [patchTeamCreate]
  );
  const setLeaderSkills = useCallback(
    (next: string[] | ((prev: string[]) => string[])) =>
      patchTeamCreate({ leaderSkills: resolveUpdater(leaderSkills, next) }),
    [leaderSkills, patchTeamCreate]
  );
  const setLeaderCustomSkills = useCallback(
    (next: string) => patchTeamCreate({ leaderCustomSkills: next }),
    [patchTeamCreate]
  );
  const setWorkers = useCallback(
    (next: WorkerDraft[] | ((prev: WorkerDraft[]) => WorkerDraft[])) =>
      patchTeamCreate({ workers: resolveUpdater(workers, next) }),
    [patchTeamCreate, workers]
  );
  const setForgeAgentBindTarget = useCallback(
    (next: TeamForgeBindTarget) => patchTeamCreate({ forgeAgentBindTarget: next }),
    [patchTeamCreate]
  );
  const setShowForgeAgentForm = useCallback(
    (next: boolean) => patchTeamCreate({ showForgeAgentForm: next }),
    [patchTeamCreate]
  );
  const setForgeAgentName = useCallback(
    (next: string) => patchTeamCreate({ forgeAgentName: next }),
    [patchTeamCreate]
  );
  const setForgeAgentWorkdir = useCallback(
    (next: string | ((prev: string) => string)) =>
      patchTeamCreate({ forgeAgentWorkdir: resolveUpdater(forgeAgentWorkdir, next) }),
    [forgeAgentWorkdir, patchTeamCreate]
  );
  const setForgeAgentPresetId = useCallback(
    (next: AgentPresetId) => patchTeamCreate({ forgeAgentPresetId: next }),
    [patchTeamCreate]
  );
  const setForgeAgentWorktreeMode = useCallback(
    (next: "use_existing" | "create_worktree" | "reuse_worktree") =>
      patchTeamCreate({ forgeAgentWorktreeMode: next }),
    [patchTeamCreate]
  );
  const setForgeAgentWorktreeRepo = useCallback(
    (next: string) => patchTeamCreate({ forgeAgentWorktreeRepo: next }),
    [patchTeamCreate]
  );
  const setForgeAgentWorktreeRef = useCallback(
    (next: string) => patchTeamCreate({ forgeAgentWorktreeRef: next }),
    [patchTeamCreate]
  );
  const setForgeAgentCodeMode = useCallback(
    (next: boolean) => patchTeamCreate({ forgeAgentCodeMode: next }),
    [patchTeamCreate]
  );
  const setForgeAgentWorktreeError = useCallback(
    (next: string | null) => patchTeamCreate({ forgeAgentWorktreeError: next }),
    [patchTeamCreate]
  );
  const setForgeAgentBusy = useCallback(
    (next: boolean) => patchTeamCreate({ forgeAgentBusy: next }),
    [patchTeamCreate]
  );
  const setTeamForgeAgentIds = useCallback(
    (next: string[] | ((prev: string[]) => string[])) =>
      patchTeamCreate({ teamForgeAgentIds: resolveUpdater(teamForgeAgentIds, next) }),
    [patchTeamCreate, teamForgeAgentIds]
  );
  const handleForgeWorktreeModeChange = useCallback(
    (nextMode: "use_existing" | "create_worktree" | "reuse_worktree") => {
      setForgeAgentWorktreeMode(nextMode);
      setForgeAgentWorkdir((prev) =>
        resolveWorkdirForModeChange(
          prev,
          nextMode,
          forgeDefaultWorktreeRoot,
          DEFAULT_WORKTREE_ROOT
        )
      );
    },
    [forgeDefaultWorktreeRoot, setForgeAgentWorkdir, setForgeAgentWorktreeMode]
  );

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

  const [steps, setSteps] = useState<TeamStepRecord[]>([]);

  const [teamMailboxState, dispatchTeamMailbox] = useReducer(
    reduceTeamMailboxState,
    DEFAULT_TEAM_MAILBOX_STATE
  );
  const msgFromActorId = teamMailboxState.msgFromActorId;
  const msgToActorId = teamMailboxState.msgToActorId;
  const msgChannel = teamMailboxState.msgChannel;
  const msgTransport = teamMailboxState.msgTransport;
  const msgRoute = teamMailboxState.msgRoute;
  const msgTemplate = teamMailboxState.msgTemplate;
  const msgPayload = teamMailboxState.msgPayload;
  const msgIdempotencyKey = teamMailboxState.msgIdempotencyKey;
  const chatDraft = teamMailboxState.chatDraft;
  const chatStickToBottom = teamMailboxState.chatStickToBottom;
  const chatSeenByConversation = teamMailboxState.chatSeenByConversation;
  const inboxActorId = teamMailboxState.inboxActorId;
  const inboxLimit = teamMailboxState.inboxLimit;
  const inboxAfterId = teamMailboxState.inboxAfterId;
  const inboxIncludeDelivered = teamMailboxState.inboxIncludeDelivered;
  const inbox = teamMailboxState.inbox;
  const selectedMemberId = teamMailboxState.selectedMemberId;
  const patchTeamMailbox = useCallback((patch: Partial<TeamMailboxState>) => {
    dispatchTeamMailbox({ type: "patch", patch });
  }, []);
  const setMsgFromActorId = useCallback(
    (next: string) => patchTeamMailbox({ msgFromActorId: next }),
    [patchTeamMailbox]
  );
  const setMsgToActorId = useCallback(
    (next: string) => patchTeamMailbox({ msgToActorId: next }),
    [patchTeamMailbox]
  );
  const setMsgChannel = useCallback(
    (next: string) => patchTeamMailbox({ msgChannel: next }),
    [patchTeamMailbox]
  );
  const setMsgTransport = useCallback(
    (next: "local" | "remote") => patchTeamMailbox({ msgTransport: next }),
    [patchTeamMailbox]
  );
  const setMsgRoute = useCallback(
    (next: string) => patchTeamMailbox({ msgRoute: next }),
    [patchTeamMailbox]
  );
  const setMsgTemplate = useCallback(
    (next: MailboxTemplateKey) => patchTeamMailbox({ msgTemplate: next }),
    [patchTeamMailbox]
  );
  const setMsgPayload = useCallback(
    (next: string) => patchTeamMailbox({ msgPayload: next }),
    [patchTeamMailbox]
  );
  const setMsgIdempotencyKey = useCallback(
    (next: string) => patchTeamMailbox({ msgIdempotencyKey: next }),
    [patchTeamMailbox]
  );
  const setChatDraft = useCallback(
    (next: string) => patchTeamMailbox({ chatDraft: next }),
    [patchTeamMailbox]
  );
  const setChatStickToBottom = useCallback(
    (next: boolean) => patchTeamMailbox({ chatStickToBottom: next }),
    [patchTeamMailbox]
  );
  const setChatSeenByConversation = useCallback(
    (next: Record<string, number>) => patchTeamMailbox({ chatSeenByConversation: next }),
    [patchTeamMailbox]
  );
  const setInboxActorId = useCallback(
    (next: string) => patchTeamMailbox({ inboxActorId: next }),
    [patchTeamMailbox]
  );
  const setInboxLimit = useCallback(
    (next: string) => patchTeamMailbox({ inboxLimit: next }),
    [patchTeamMailbox]
  );
  const setInboxAfterId = useCallback(
    (next: string) => patchTeamMailbox({ inboxAfterId: next }),
    [patchTeamMailbox]
  );
  const setInboxIncludeDelivered = useCallback(
    (next: boolean) => patchTeamMailbox({ inboxIncludeDelivered: next }),
    [patchTeamMailbox]
  );
  const setInbox = useCallback(
    (next: TeamActorMessageRecord[]) => patchTeamMailbox({ inbox: next }),
    [patchTeamMailbox]
  );
  const setSelectedMemberId = useCallback(
    (next: string) => patchTeamMailbox({ selectedMemberId: next }),
    [patchTeamMailbox]
  );
  const chatMessagesRef = useRef<HTMLUListElement | null>(null);

  const eventsRef = useRef<TeamRunEventRecord[]>([]);
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
  const teamForgeAgents = useMemo(
    () => selectTeamForgeAgents(agents, teamForgeAgentIds),
    [agents, teamForgeAgentIds]
  );
  const leaderAgent = useMemo(
    () => teamForgeAgents.find((agent) => agent.id === leaderMemberId) ?? null,
    [teamForgeAgents, leaderMemberId]
  );
  const hasForgeAgents = teamForgeAgents.length > 0;

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
  const chatMemberIds = useMemo(
    () => snapshot?.members.map((member) => member.member_id) ?? [],
    [snapshot]
  );
  const chatActors = useMemo(
    () =>
      resolveMailboxChatActors(
        snapshot?.leader_member_id,
        chatMemberIds,
        selectedMemberId
      ),
    [chatMemberIds, selectedMemberId, snapshot?.leader_member_id]
  );
  const mergedMailboxMessages = useMemo(
    () => mergeMailboxMessages(snapshot?.mailbox.recent_messages ?? [], inbox),
    [inbox, snapshot?.mailbox.recent_messages]
  );
  const conversationMessages = useMemo(
    () =>
      selectMailboxConversation(
        mergedMailboxMessages,
        chatActors.fromActorId,
        chatActors.toActorId
      ),
    [chatActors.fromActorId, chatActors.toActorId, mergedMailboxMessages]
  );
  const conversationKey = useMemo(
    () => buildMailboxConversationKey(chatActors.fromActorId, chatActors.toActorId),
    [chatActors.fromActorId, chatActors.toActorId]
  );
  const conversationLatestMessageId = useMemo(
    () => resolveConversationMaxMessageId(conversationMessages),
    [conversationMessages]
  );
  const unreadByMemberId = useMemo(() => {
    if (!snapshot || chatMemberIds.length === 0) {
      return {} as Record<string, number>;
    }
    const counts: Record<string, number> = {};
    for (const member of snapshot.members) {
      const actors = resolveMailboxChatActors(
        snapshot.leader_member_id,
        chatMemberIds,
        member.member_id
      );
      const key = buildMailboxConversationKey(actors.fromActorId, actors.toActorId);
      const seenMessageId = key ? chatSeenByConversation[key] ?? 0 : 0;
      counts[member.member_id] = countUnreadConversationMessages(
        mergedMailboxMessages,
        actors.fromActorId,
        actors.toActorId,
        seenMessageId
      );
    }
    return counts;
  }, [chatMemberIds, chatSeenByConversation, mergedMailboxMessages, snapshot]);
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
    return teamForgeAgents.filter((agent) => !used.has(agent.id)).length;
  }, [leaderMemberId, teamForgeAgents, workerAgentIds]);
  const isMissionBriefReady = useMemo(
    () => newTeamName.trim().length > 0,
    [newTeamName]
  );
  const isLeaderForgeReady = useMemo(
    () =>
      useSpecOverride ||
      leaderMemberId.trim().length > 0 &&
      teamForgeAgents.some((agent) => agent.id === leaderMemberId),
    [leaderMemberId, teamForgeAgents, useSpecOverride]
  );
  const isRecruitWorkersReady = useMemo(
    () => useSpecOverride || !hasDuplicateMembers,
    [hasDuplicateMembers, useSpecOverride]
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
        label: useSpecOverride
          ? "Leader/worker forge skipped (manual spec mode)"
          : "Leader selected",
        ready: isLeaderForgeReady,
      },
      {
        key: "party",
        label: useSpecOverride
          ? "Member assignments provided in manual spec JSON"
          : hasDuplicateMembers
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
  const leaderForgeAgentOptions = useMemo(
    () =>
      teamForgeAgents.map((agent) => ({
        value: agent.id,
        label: buildAgentLabel(agent),
      })),
    [teamForgeAgents]
  );
  const leaderAgentSelectOptions = useMemo(() => {
    const options = [...leaderForgeAgentOptions];
    const hasSelected = options.some((option) => option.value === leaderMemberId);
    if (leaderMemberId && !hasSelected) {
      options.unshift({
        value: leaderMemberId,
        label: `Missing forged agent (${leaderMemberId})`,
      });
    }
    return options;
  }, [leaderForgeAgentOptions, leaderMemberId]);

  const oldestEventId = events.length > 0 ? events[0].event_id : null;
  const oldestMemberEventId =
    memberEvents.length > 0 ? memberEvents[0].event_id : null;

  const resetTeamDraft = useCallback(() => {
    const initial = createInitialTeamDraftState();
    patchTeamCreate({
      newTeamName: "",
      newTeamDescription: "",
      leaderMemberId: initial.leaderMemberId,
      leaderModel: initial.leaderModel,
      leaderPrompt: initial.leaderPrompt,
      leaderSkills: initial.leaderSkills,
      leaderCustomSkills: initial.leaderCustomSkills,
      workers: initial.workers,
      useSpecOverride: initial.useSpecOverride,
      newTeamSpec: initial.newTeamSpec,
      teamForgeAgentIds: initial.teamForgeAgentIds,
      forgeAgentBindTarget: "none",
      showForgeAgentForm: false,
      forgeAgentName: "",
      forgeAgentWorkdir: "",
      forgeAgentPresetId: DEFAULT_AGENT_PRESET_ID,
      forgeAgentWorktreeMode: "use_existing",
      forgeAgentWorktreeRepo: "",
      forgeAgentWorktreeRef: "",
      forgeAgentCodeMode: true,
      forgeAgentWorktreeError: null,
      forgeAgentBusy: false,
    });
  }, [patchTeamCreate]);

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
    [props.token, setSelectedStepId]
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

  const loadInbox = useCallback(async (actorIdOverride?: string) => {
    if (!activeRunId) return;
    const actorId = (actorIdOverride ?? inboxActorId).trim();
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
    setInbox,
  ]);

  const markConversationSeen = useCallback(
    (key: string, messageId: number | null) => {
      if (!key || messageId == null) {
        return;
      }
      dispatchTeamMailbox({
        type: "mark_conversation_seen",
        key,
        messageId,
      });
    },
    []
  );

  const scrollConversationToBottom = useCallback(() => {
    const el = chatMessagesRef.current;
    if (!el) {
      return;
    }
    el.scrollTop = el.scrollHeight;
  }, []);

  const onConversationScroll = useCallback(() => {
    const el = chatMessagesRef.current;
    if (!el) {
      return;
    }
    const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
    const stick = gap <= 24;
    setChatStickToBottom(stick);
    if (stick) {
      markConversationSeen(conversationKey, conversationLatestMessageId);
    }
  }, [
    conversationKey,
    conversationLatestMessageId,
    markConversationSeen,
    setChatStickToBottom,
  ]);

  const onJumpConversationToBottom = useCallback(() => {
    setChatStickToBottom(true);
    window.requestAnimationFrame(() => {
      scrollConversationToBottom();
      markConversationSeen(conversationKey, conversationLatestMessageId);
    });
  }, [
    conversationKey,
    conversationLatestMessageId,
    markConversationSeen,
    scrollConversationToBottom,
    setChatStickToBottom,
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
  }, [refreshTeamRuns, runStatusFilter, selectedTeamId, setInbox, setSelectedMemberId]);

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
      setChatSeenByConversation({});
      setChatStickToBottom(true);
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
    setChatSeenByConversation,
    setChatStickToBottom,
    setInbox,
    setSelectedMemberId,
    selectedTeamId,
  ]);

  useEffect(() => {
    if (!activeRunId || !eventsAutoRefresh) return;
    const timer = window.setInterval(() => {
      if (tab === "mailbox") {
        void refreshSnapshot(activeRunId).catch(() => undefined);
        const actorId = chatActors.inboxActorId.trim();
        if (actorId) {
          void loadInbox(actorId).catch(() => undefined);
        }
        return;
      }
      void refreshRun(activeRunId).catch(() => undefined);
      void refreshEvents(activeRunId).catch(() => undefined);
      void refreshSnapshot(activeRunId).catch(() => undefined);
    }, 4000);
    return () => {
      window.clearInterval(timer);
    };
  }, [
    activeRunId,
    chatActors.inboxActorId,
    eventsAutoRefresh,
    loadInbox,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    tab,
  ]);

  useEffect(() => {
    if (!snapshot) {
      setSelectedMemberId("");
      setMemberEvents([]);
      return;
    }
    if (
      selectedMemberId &&
      snapshot.members.some((member) => member.member_id === selectedMemberId)
    ) {
      return;
    }
    setSelectedMemberId(snapshot.members[0]?.member_id ?? "");
  }, [selectedMemberId, setSelectedMemberId, snapshot]);

  useEffect(() => {
    const actorId = chatActors.inboxActorId.trim();
    if (!activeRunId || !actorId) {
      setInbox([]);
      return;
    }
    setInboxActorId(actorId);
    void loadInbox(actorId).catch((err) => {
      setError(parseErrorMessage(err));
    });
  }, [activeRunId, chatActors.inboxActorId, loadInbox, setInbox, setInboxActorId]);

  useEffect(() => {
    if (tab !== "mailbox") {
      return;
    }
    setChatStickToBottom(true);
    window.requestAnimationFrame(() => {
      scrollConversationToBottom();
    });
  }, [conversationKey, scrollConversationToBottom, setChatStickToBottom, tab]);

  useEffect(() => {
    if (tab !== "mailbox" || !chatStickToBottom) {
      return;
    }
    window.requestAnimationFrame(() => {
      scrollConversationToBottom();
      markConversationSeen(conversationKey, conversationLatestMessageId);
    });
  }, [
    chatStickToBottom,
    conversationKey,
    conversationLatestMessageId,
    conversationMessages.length,
    markConversationSeen,
    scrollConversationToBottom,
    tab,
  ]);

  useEffect(() => {
    void loadMemberEvents("replace").catch((err) => {
      setError(parseErrorMessage(err));
    });
  }, [loadMemberEvents]);

  useEffect(() => {
    if (!props.token) {
      setForgeDefaultWorktreeRoot(DEFAULT_WORKTREE_ROOT);
      return;
    }
    api
      .getRuntimeDefaults(props.token)
      .then((defaults) => {
        const root = normalizeRuntimeWorktreeRoot(
          defaults.default_worktree_root,
          DEFAULT_WORKTREE_ROOT
        );
        setForgeDefaultWorktreeRoot(root);
      })
      .catch(() => undefined);
  }, [props.token]);

  useEffect(() => {
    if (!showCreateTeamModal) return;
    if (leaderMemberId && teamForgeAgents.some((agent) => agent.id === leaderMemberId)) {
      return;
    }
    const fallbackLeaderId = teamForgeAgents[0]?.id ?? "";
    setLeaderMemberId(fallbackLeaderId);
  }, [leaderMemberId, setLeaderMemberId, showCreateTeamModal, teamForgeAgents]);

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
  }, [busy, setCreateTeamStage, setShowCreateTeamModal, showCreateTeamModal]);

  const openCreateTeamModal = () => {
    setError(null);
    setCreateTeamStage(0);
    resetTeamDraft();
    setShowCreateTeamModal(true);
    setShowForgeAgentForm(false);
    setForgeAgentWorktreeError(null);
    void refreshAgents().catch((err) => {
      setError(parseErrorMessage(err));
    });
  };

  const closeCreateTeamModal = () => {
    setShowCreateTeamModal(false);
    setCreateTeamStage(0);
    setShowForgeAgentForm(false);
    setForgeAgentWorktreeError(null);
  };

  const openForgeAgentForm = () => {
    setError(null);
    setForgeAgentWorktreeError(null);
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
    setForgeAgentWorktreeMode("use_existing");
    setForgeAgentWorkdir((prev) =>
      resolveWorkdirForModalOpen(
        prev,
        "use_existing",
        forgeDefaultWorktreeRoot,
        DEFAULT_WORKTREE_ROOT
      )
    );
    setForgeAgentWorktreeRepo("");
    setForgeAgentWorktreeRef("");
    setForgeAgentPresetId(DEFAULT_AGENT_PRESET_ID);
    setForgeAgentCodeMode(true);
  };

  const closeForgeAgentForm = () => {
    if (forgeAgentBusy) return;
    setShowForgeAgentForm(false);
    setForgeAgentWorktreeError(null);
  };

  const onCreateForgeAgent = async () => {
    if (forgeAgentBusy) return;
    const name = forgeAgentName.trim() || "agent";
    const workdir = normalizeWorkdirInput(forgeAgentWorkdir);
    const normalizedRoot = normalizeWorkdirInput(forgeDefaultWorktreeRoot);
    const workdirPayload =
      forgeAgentWorktreeMode === "create_worktree" &&
      normalizedRoot &&
      workdir === normalizedRoot
        ? ""
        : workdir;
    if (!workdirPayload && forgeAgentWorktreeMode !== "create_worktree") {
      setError("Forge agent workdir is required");
      return;
    }
    if (forgeAgentWorktreeMode !== "use_existing" && !forgeAgentWorktreeRepo.trim()) {
      setError("Worktree repo is required");
      return;
    }
    setForgeAgentBusy(true);
    setError(null);
    setForgeAgentWorktreeError(null);
    try {
      const preset = getAgentPreset(forgeAgentPresetId);
      const created = await api.createAgent(props.token, {
        name,
        workdir: workdirPayload,
        command: preset.command,
        args: preset.args.slice(),
        worktree_mode: forgeAgentWorktreeMode,
        worktree_repo: forgeAgentWorktreeRepo.trim() || null,
        worktree_ref: forgeAgentWorktreeRef.trim() || null,
        code_mode: forgeAgentCodeMode,
      });
      setAgents((prev) => [created, ...prev.filter((agent) => agent.id !== created.id)]);
      setTeamForgeAgentIds((prev) =>
        prev.includes(created.id) ? prev : [...prev, created.id]
      );
      if (forgeAgentBindTarget === "leader") {
        setLeaderMemberId(created.id);
      } else if (forgeAgentBindTarget === "worker") {
        setWorkers((prev) => assignCreatedWorkerToDraft(prev, created.id));
      }
      setShowForgeAgentForm(false);
      setForgeAgentWorktreeError(null);
    } catch (err) {
      const hint = formatTeamForgeWorktreeError(err);
      setForgeAgentWorktreeError(hint);
      setError(hint ?? parseErrorMessage(err));
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
    setCreateTeamStage((prev) => {
      if (useSpecOverride && prev === 0) {
        return 3;
      }
      return clampCreateTeamStage(prev + 1);
    });
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
    if (
      !useSpecOverride &&
      !teamForgeAgents.some((agent) => agent.id === leaderMemberId.trim())
    ) {
      setError("Leader must be selected from Team Forge agents");
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
      resetTeamDraft();
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

  const onRunStatusFilterChange = useCallback(
    (nextFilter: TeamRunStatusFilter) => {
      if (!selectedTeamId) return;
      setTeamRunBrowserByTeam((prev) => ({
        ...prev,
        [selectedTeamId]: {
          statusFilter: nextFilter,
          beforeCreatedAt: undefined,
          hasMore: false,
        },
      }));
    },
    [selectedTeamId]
  );

  const onRefreshRuns = useCallback(async () => {
    if (!selectedTeamId) return;
    setError(null);
    try {
      await refreshTeamRuns(selectedTeamId, "replace", {
        statusFilter: runStatusFilter,
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [refreshTeamRuns, runStatusFilter, selectedTeamId]);

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

  const onSendChatMessage = async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    const fromActorId = chatActors.fromActorId.trim();
    const toActorId = chatActors.toActorId.trim();
    const text = chatDraft.trim();
    if (!fromActorId || !toActorId) {
      setError("Select a valid member conversation first");
      return;
    }
    if (!text) {
      setError("Chat message is required");
      return;
    }
    setBusy("send-chat");
    setError(null);
    try {
      await api.sendTeamRunMessage(props.token, activeRunId, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: "default",
        transport: "local",
        payload: buildMailboxChatPayload(text),
      });
      setChatDraft("");
      await refreshSnapshot(activeRunId);
      await loadInbox(toActorId);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
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
      if (tab === "mailbox") {
        await refreshSnapshot(activeRunId);
        if (inboxActorId.trim()) {
          await loadInbox();
        }
      } else {
        await Promise.all([refreshEvents(activeRunId), refreshSnapshot(activeRunId)]);
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
      if (tab === "mailbox") {
        await Promise.all([loadInbox(actorId), refreshSnapshot(activeRunId)]);
      } else {
        await Promise.all([
          loadInbox(),
          refreshEvents(activeRunId),
          refreshSnapshot(activeRunId),
        ]);
      }
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onRefreshMemberConsole = useCallback(async () => {
    if (selectedMemberSnapshot) {
      await loadMemberEvents("replace");
      return;
    }
    if (activeRunId) {
      await refreshEvents(activeRunId);
    }
  }, [activeRunId, loadMemberEvents, refreshEvents, selectedMemberSnapshot]);

  const onLoadOlderMemberConsole = useCallback(async () => {
    if (!selectedMemberSnapshot) {
      return;
    }
    await loadMemberEvents("prepend");
  }, [loadMemberEvents, selectedMemberSnapshot]);

  const onRefreshOverviewSnapshot = useCallback(async () => {
    if (!activeRunId) return;
    setError(null);
    try {
      await refreshSnapshot(activeRunId);
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRunId, refreshSnapshot]);

  const onOpenMailboxForMember = useCallback((memberId: string) => {
    setSelectedMemberId(memberId);
    setTab("mailbox");
  }, [setSelectedMemberId, setTab]);

  const onRefreshEventsPanel = useCallback(async () => {
    if (!activeRun) return;
    setError(null);
    try {
      await refreshEvents(activeRun.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRun, refreshEvents]);

  const onLoadOlderEventsPanel = useCallback(async () => {
    if (!activeRun) return;
    setError(null);
    try {
      await refreshEvents(activeRun.id, "prepend");
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRun, refreshEvents]);

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
              skills: toggleSkillSelection(
                worker.skills,
                skill,
                REQUIRED_TEAM_WORKER_SKILLS
              ),
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
      const memberId = pickNextWorkerAgentId(teamForgeAgents, excluded);
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
      for (const agent of teamForgeAgents) {
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
        const replacement = pickNextWorkerAgentId(teamForgeAgents, used);
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
        <TeamSidebar
          busy={busy}
          onRefreshTeams={refreshTeams}
          onOpenCreateTeamModal={openCreateTeamModal}
          draftTeamName={newTeamName}
          leaderMemberId={leaderMemberId}
          configuredWorkerCount={configuredWorkerCount}
          teams={teams}
          selectedTeamId={selectedTeamId}
          teamMemberSummaryByTeamId={teamMemberSummaryByTeamId}
          onSelectTeam={(teamId) => {
            setSelectedTeamId(teamId);
            setRunLookupId("");
          }}
        />

        <div className="teams-main">
          {!selectedTeam && (
            <div className="card">
              <h2>Team Workbench</h2>
              <p>Select a team from the left panel to manage runs, steps, and messages.</p>
            </div>
          )}

          {selectedTeam && (
            <>
              <TeamRunPanel
                selectedTeam={selectedTeam}
                busy={busy}
                onDeleteTeam={onDeleteTeam}
                selectedTeamMemberSummary={selectedTeamMemberSummary}
                selectedTeamMemberLiveStates={selectedTeamMemberLiveStates}
                runContextId={runContextId}
                onRunContextIdChange={setRunContextId}
                onCreateRun={onCreateRun}
                runInput={runInput}
                onRunInputChange={setRunInput}
                runLookupId={runLookupId}
                onRunLookupIdChange={setRunLookupId}
                onLoadRunById={onLoadRunById}
                runStatusFilter={runStatusFilter}
                runStatusFilterOptions={TEAM_RUN_STATUS_FILTER_OPTIONS}
                onRunStatusFilterChange={onRunStatusFilterChange}
                onRefreshRuns={onRefreshRuns}
                runsLoading={runsLoading}
                visibleRuns={visibleRuns}
                activeRunId={activeRunId}
                onActiveRunChange={setActiveRunId}
                isActiveRunHiddenByFilter={isActiveRunHiddenByFilter}
                activeRun={activeRun}
                totalLoadedRunsForTeam={totalLoadedRunsForTeam}
                pageLimit={TEAM_RUN_PAGE_LIMIT}
                runsHasMore={runsHasMore}
                selectedTeamId={selectedTeamId}
                onLoadMoreRuns={onLoadMoreRuns}
              />

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
                        <strong>Status:</strong>{" "}
                        <StatusBadge
                          label={activeRun.status}
                          tone={resolveTeamRunStatusTone(activeRun.status)}
                          className="team-status"
                          title={`run status: ${activeRun.status}`}
                        />
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
                    <TeamOverviewPanel
                      snapshot={snapshot}
                      snapshotLoading={snapshotLoading}
                      onRefreshSnapshot={onRefreshOverviewSnapshot}
                      selectedMemberId={selectedMemberId}
                      onOpenMailboxForMember={onOpenMailboxForMember}
                    />
                  )}

                  {tab === "events" && (
                    <TeamEventsPanel
                      eventsAutoRefresh={eventsAutoRefresh}
                      onEventsAutoRefreshChange={setEventsAutoRefresh}
                      onRefreshEvents={onRefreshEventsPanel}
                      onLoadOlderEvents={onLoadOlderEventsPanel}
                      eventsLoading={eventsLoading}
                      previewMode={previewMode}
                      previewLimit={TEAM_EVENT_PREVIEW_LIMIT}
                      eventsHasMore={eventsHasMore}
                      oldestEventId={oldestEventId}
                      displayedRunEvents={displayedRunEvents}
                      formatTs={formatTs}
                      toPrettyJson={toPrettyJson}
                    />
                  )}

                  {tab === "steps" && (
                    <TeamStepsPanel
                      steps={steps}
                      onRefreshSteps={async () => {
                        await refreshSteps(activeRun.id);
                      }}
                      stepKey={stepKey}
                      onStepKeyChange={setStepKey}
                      stepMemberId={stepMemberId}
                      onStepMemberIdChange={setStepMemberId}
                      stepDependsOn={stepDependsOn}
                      onStepDependsOnChange={setStepDependsOn}
                      stepInput={stepInput}
                      onStepInputChange={setStepInput}
                      onSubmitStep={onSubmitStep}
                      busy={busy}
                      selectedStepId={selectedStepId}
                      onSelectedStepIdChange={setSelectedStepId}
                      stepAction={stepAction}
                      onStepActionChange={setStepAction}
                      stepRemoteTaskId={stepRemoteTaskId}
                      onStepRemoteTaskIdChange={setStepRemoteTaskId}
                      stepOutput={stepOutput}
                      onStepOutputChange={setStepOutput}
                      stepFailText={stepFailText}
                      onStepFailTextChange={setStepFailText}
                      stepInputReason={stepInputReason}
                      onStepInputReasonChange={setStepInputReason}
                      stepInputRequiredPayload={stepInputRequiredPayload}
                      onStepInputRequiredPayloadChange={setStepInputRequiredPayload}
                      stepResumePayload={stepResumePayload}
                      onStepResumePayloadChange={setStepResumePayload}
                      onApplyStepAction={onApplyStepAction}
                    />
                  )}

                  {tab === "mailbox" && (
                    <TeamMailboxPanel
                      snapshot={snapshot}
                      selectedMemberId={selectedMemberId}
                      unreadByMemberId={unreadByMemberId}
                      onSelectMember={setSelectedMemberId}
                      chatActors={chatActors}
                      chatStickToBottom={chatStickToBottom}
                      chatMessagesRef={chatMessagesRef}
                      onConversationScroll={onConversationScroll}
                      onJumpToBottom={onJumpConversationToBottom}
                      conversationMessages={conversationMessages}
                      toPrettyJson={toPrettyJson}
                      formatTs={formatTs}
                      busy={busy}
                      onAckMessage={onAckMessage}
                      chatDraft={chatDraft}
                      onChatDraftChange={setChatDraft}
                      onSendChatMessage={onSendChatMessage}
                      msgFromActorId={msgFromActorId}
                      onMsgFromActorIdChange={setMsgFromActorId}
                      msgToActorId={msgToActorId}
                      onMsgToActorIdChange={setMsgToActorId}
                      msgChannel={msgChannel}
                      onMsgChannelChange={setMsgChannel}
                      msgTransport={msgTransport}
                      onMsgTransportChange={setMsgTransport}
                      msgRoute={msgRoute}
                      onMsgRouteChange={setMsgRoute}
                      mailboxTemplateOptions={MAILBOX_TEMPLATE_OPTIONS}
                      msgTemplate={msgTemplate}
                      onMsgTemplateChange={(value) =>
                        setMsgTemplate(value as MailboxTemplateKey)
                      }
                      onApplyMessageTemplate={onApplyMessageTemplate}
                      msgPayload={msgPayload}
                      onMsgPayloadChange={setMsgPayload}
                      msgIdempotencyKey={msgIdempotencyKey}
                      onMsgIdempotencyKeyChange={setMsgIdempotencyKey}
                      onSendMessage={onSendMessage}
                      inboxActorId={inboxActorId}
                      onInboxActorIdChange={setInboxActorId}
                      inboxLimit={inboxLimit}
                      onInboxLimitChange={setInboxLimit}
                      inboxAfterId={inboxAfterId}
                      onInboxAfterIdChange={setInboxAfterId}
                      inboxIncludeDelivered={inboxIncludeDelivered}
                      onInboxIncludeDeliveredChange={setInboxIncludeDelivered}
                      onRefreshInbox={onRefreshInbox}
                    />
                  )}

                  {tab === "member_console" && (
                    <TeamMemberConsolePanel
                      snapshot={snapshot}
                      selectedMemberId={selectedMemberId}
                      onSelectedMemberIdChange={setSelectedMemberId}
                      selectedMemberSnapshot={selectedMemberSnapshot}
                      memberEvents={memberEvents}
                      memberEventsHasMore={memberEventsHasMore}
                      memberEventsLoading={memberEventsLoading}
                      eventsLoading={eventsLoading}
                      oldestMemberEventId={oldestMemberEventId}
                      displayedRunEvents={displayedRunEvents}
                      previewLimit={TEAM_EVENT_PREVIEW_LIMIT}
                      onRefresh={onRefreshMemberConsole}
                      onLoadOlder={onLoadOlderMemberConsole}
                      toPrettyJson={toPrettyJson}
                      formatTs={formatTs}
                    />
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
                <div className="team-create-forge-agent-meta mono">
                  <span>bind_target</span>
                  <select
                    value={forgeAgentBindTarget}
                    onChange={(event) =>
                      setForgeAgentBindTarget(event.target.value as TeamForgeBindTarget)
                    }
                    disabled={forgeAgentBusy}
                  >
                    <option value="none">none</option>
                    <option value="leader">leader</option>
                    <option value="worker">worker</option>
                  </select>
                </div>
                {showForgeAgentForm && (
                  <div className="team-create-stage-note">
                    Agent create modal is open. Complete fields there to forge and bind agent.
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
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={useSpecOverride}
                      onChange={(event) => setUseSpecOverride(event.target.checked)}
                    />
                    Manual spec mode (skip Leader/Workers wizard stages)
                  </label>
                  {useSpecOverride && (
                    <p className="team-create-stage-note">
                      Manual spec mode is enabled. Next step jumps directly to Launch Team where
                      you can edit JSON spec.
                    </p>
                  )}
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
                    Choose the leader from agents created in this Team Forge session only.
                  </p>
                  {!isLeaderForgeReady && hasForgeAgents && (
                    <p className="team-create-stage-note">
                      Select one forged leader agent to continue.
                    </p>
                  )}
                  {!hasForgeAgents && (
                    <p className="muted">
                      No forged agents yet. Create one in the Agent Forge entry above.
                    </p>
                  )}
                  <select
                    value={leaderMemberId}
                    onChange={(event) => setLeaderMemberId(event.target.value)}
                    disabled={useSpecOverride || !hasForgeAgents}
                  >
                    <option value="">Select forged leader agent</option>
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
                      const isRequired = REQUIRED_TEAM_LEADER_SKILLS.includes(skill);
                      return (
                        <button
                          key={`leader-skill-${skill}`}
                          type="button"
                          className={selected ? "team-skill-tag selected" : "team-skill-tag"}
                          onClick={() =>
                            setLeaderSkills((prev) =>
                              toggleSkillSelection(
                                prev,
                                skill,
                                REQUIRED_TEAM_LEADER_SKILLS
                              )
                            )
                          }
                          disabled={useSpecOverride || isRequired}
                          title={isRequired ? "Required for leader role" : undefined}
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
                      <button onClick={onAddWorker} disabled={useSpecOverride || !hasForgeAgents}>
                        Add Worker
                      </button>
                      <button
                        onClick={onAddAllRemainingWorkers}
                        disabled={
                          useSpecOverride || !hasForgeAgents || availableWorkerAgentCount === 0
                        }
                        type="button"
                      >
                        Auto Fill Party
                      </button>
                    </div>
                  </div>
                  <p className="muted">
                    Build your party from Team Forge agents only. Worker model/prompt/skills can
                    still be customized at team level.
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
                      const workerOptions = leaderForgeAgentOptions.filter((option) => {
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
                            disabled={useSpecOverride || !hasForgeAgents}
                          >
                            <option value="">Select forged worker agent</option>
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
                              const isRequired = REQUIRED_TEAM_WORKER_SKILLS.includes(skill);
                              return (
                                <button
                                  key={`worker-skill-${index}-${skill}`}
                                  type="button"
                                  className={
                                    selected ? "team-skill-tag selected" : "team-skill-tag"
                                  }
                                  onClick={() => onToggleWorkerSkill(index, skill)}
                                  disabled={useSpecOverride || isRequired}
                                  title={isRequired ? "Required for worker role" : undefined}
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
                      (!hasForgeAgents || !leaderMemberId.trim() || hasDuplicateMembers))
                  }
                  type="button"
                >
                  Create Team
                </button>
              )}
            </div>
          </div>
          {showForgeAgentForm && (
            <CreateAgentModal
              agentName={forgeAgentName}
              setAgentName={setForgeAgentName}
              agentWorkdir={forgeAgentWorkdir}
              setAgentWorkdir={setForgeAgentWorkdir}
              agentPresetId={forgeAgentPresetId}
              setAgentPresetId={setForgeAgentPresetId}
              worktreeMode={forgeAgentWorktreeMode}
              setWorktreeMode={handleForgeWorktreeModeChange}
              worktreeRepo={forgeAgentWorktreeRepo}
              setWorktreeRepo={setForgeAgentWorktreeRepo}
              worktreeRef={forgeAgentWorktreeRef}
              setWorktreeRef={setForgeAgentWorktreeRef}
              codeMode={forgeAgentCodeMode}
              setCodeMode={setForgeAgentCodeMode}
              worktreeError={forgeAgentWorktreeError}
              createBusy={forgeAgentBusy}
              workdirPlaceholder={forgeDefaultWorktreeRoot}
              withinPortal
              onCreateAgent={onCreateForgeAgent}
              onClose={closeForgeAgentForm}
            />
          )}
        </div>
      )}
    </div>
  );
}
