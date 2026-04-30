import { formatAgentModelLabel } from "../../agent_presets";
import type {
  AgentEvent,
  AgentRecord,
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
  TeamRuntimeControlResponse,
  TeamRuntimeRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamTaskRecord,
} from "../../api";
import type { StatusTone } from "../../components/status_badge";
import { mergeOutputsPreserveHistory } from "../../output_cache";
import { normalizeTeamMemberLifecycle, normalizeTeamMemberWorkStatus } from "../team_member_status_strip";
import type {
  TeamMemberAgentStatus,
  TeamMemberAgentStatusSummary,
  TeamMemberLiveState,
} from "./member_helpers";

function sortRuns(runs: TeamRunRecord[]): TeamRunRecord[] {
  return [...runs].sort((a, b) => b.created_at - a.created_at);
}

export const DEFAULT_TEAM_THREAD_TITLE = "all";
export const DEFAULT_TEAM_THREAD_BOOTSTRAP_KIND = "shared_thread";
export const TEAM_CHANNEL_TASK_BOOTSTRAP_KIND = "team_channel";
export const TEAM_CONVERSATION_MESSAGE_RETENTION_LIMIT = 60;
export const TEAM_RUN_EVENT_RETENTION_LIMIT = 100;
export const TEAM_MEMBER_EVENT_RETENTION_LIMIT = 300;

function collectMemberIds(
  members?: Array<{ member_id?: string | null }> | null
): string[] {
  const ids = members
    ?.map((member) => member.member_id?.trim() ?? "")
    .filter(Boolean) ?? [];
  return [...new Set(ids)];
}

export type TeamRuntimeStatusView = {
  status: "running" | "stopped" | "degraded";
  label: string;
  tone: StatusTone;
  online: number;
  total: number;
};

export type TeamRuntimeControlTone = {
  statusColor: "teal" | "yellow" | "gray";
  countColor: "teal" | "yellow" | "gray";
};

export type TeamPageNotice =
  | { kind: "runtime"; title: string; message: string }
  | { kind: "warning"; title: string; message: string };

export type AgentWorkspaceStatusView = {
  role: string;
  lifecycle: string;
  work: string;
  inbox: string;
  currentWork: string;
};

export type TeamMemberAgentControlState = {
  canStart: boolean;
  canStop: boolean;
  canDelete: boolean;
};

export function isCurrentTeamScopedRequest(
  current: { teamId: string; requestSeq: number },
  teamId: string,
  requestSeq: number
): boolean {
  return Boolean(teamId) && current.teamId === teamId && current.requestSeq === requestSeq;
}

type TeamRuntimeStatusRecord = {
  status: TeamRuntimeRecord["status"];
  members: Array<Pick<TeamRuntimeRecord["members"][number], "member_id" | "session_id">>;
};

export function resolveSelectedAgentWorkspaceLabel(
  selectedMemberId: string,
  member: TeamMemberLiveState | null,
  fallbackAgentName?: string | null
): string {
  const liveAgentName = member?.agent_name?.trim();
  if (liveAgentName) {
    return liveAgentName;
  }
  const fallbackName = fallbackAgentName?.trim();
  if (fallbackName) {
    return fallbackName;
  }
  const memberId = selectedMemberId.trim();
  if (memberId) {
    return memberId;
  }
  return "Agent";
}

type ShouldClearSelectedConversationTaskArgs = {
  selectedConversationTaskId: string;
  sharedConversationTaskId: string | null;
  selectedConversationDetailPresent: boolean;
  selectedConversationDetailMissing: boolean;
  tasksLoading: boolean;
};

export function shouldClearSelectedConversationTask({
  selectedConversationTaskId,
  sharedConversationTaskId,
  selectedConversationDetailPresent,
  selectedConversationDetailMissing,
  tasksLoading,
}: ShouldClearSelectedConversationTaskArgs): boolean {
  const normalizedSelectedTaskId = selectedConversationTaskId.trim();
  if (!normalizedSelectedTaskId) {
    return false;
  }
  if (normalizedSelectedTaskId === (sharedConversationTaskId?.trim() ?? "")) {
    return false;
  }
  if (tasksLoading || selectedConversationDetailPresent || !selectedConversationDetailMissing) {
    return false;
  }
  return true;
}

type ShouldClearSelectedTeamMemberArgs = {
  selectedMemberId: string;
  memberIds: string[];
};

export function shouldClearSelectedTeamMember({
  selectedMemberId,
  memberIds,
}: ShouldClearSelectedTeamMemberArgs): boolean {
  const normalizedSelectedMemberId = selectedMemberId.trim();
  if (!normalizedSelectedMemberId) {
    return false;
  }
  if (memberIds.length === 0) {
    return false;
  }
  return !memberIds.includes(normalizedSelectedMemberId);
}

type ResolveSelectedAgentWorkspaceMemberIdArgs = {
  selectedMemberId: string;
  focusedAgentMemberId: string;
  routeSelectedMemberId?: string | null;
  knownMemberIds?: string[] | null;
};

export function resolveSelectedAgentWorkspaceMemberId({
  selectedMemberId,
  focusedAgentMemberId,
  routeSelectedMemberId,
  knownMemberIds,
}: ResolveSelectedAgentWorkspaceMemberIdArgs): string {
  const normalizedSelectedMemberId = selectedMemberId.trim();
  if (normalizedSelectedMemberId) {
    return normalizedSelectedMemberId;
  }
  const normalizedRouteMemberId = routeSelectedMemberId?.trim() ?? "";
  if (normalizedRouteMemberId) {
    if (knownMemberIds?.includes(normalizedRouteMemberId)) {
      return normalizedRouteMemberId;
    }
    return "";
  }
  return focusedAgentMemberId.trim();
}

export function resolveAgentWorkspaceStatusView(
  member: TeamMemberLiveState | null
): AgentWorkspaceStatusView {
  const lifecycle = member ? normalizeTeamMemberLifecycle(member) : "unknown";
  const workStatus = member ? normalizeTeamMemberWorkStatus(member) : "unknown";
  return {
    role: member?.role ?? "-",
    lifecycle,
    work: workStatus === "no_run" ? "no run" : workStatus,
    inbox:
      member?.pending_inbox_count == null ? "-" : String(member.pending_inbox_count),
    currentWork:
      member?.current_work?.trim() || "No direct activity reported yet.",
  };
}

export function resolveTeamMemberAgentControlState(
  agent: Pick<AgentRecord, "id"> | null,
  lifecycle: string,
  busy: string | null
): TeamMemberAgentControlState {
  const normalizedLifecycle = lifecycle.trim().toLowerCase();
  const hasAgent = Boolean(agent?.id?.trim());
  const isRunning =
    normalizedLifecycle === "working" || normalizedLifecycle === "idle";
  return {
    canStart:
      hasAgent &&
      busy !== "start-team-member-agent" &&
      !isRunning,
    canStop:
      hasAgent &&
      busy !== "stop-team-member-agent" &&
      isRunning,
    canDelete: hasAgent && busy !== "delete-team-member-agent",
  };
}

export function removeTeamMemberLookupEntry<T>(
  lookup: Record<string, T>,
  memberId: string
): Record<string, T> {
  if (!Object.prototype.hasOwnProperty.call(lookup, memberId)) {
    return lookup;
  }
  const next = { ...lookup };
  delete next[memberId];
  return next;
}

export function resolveTeamRuntimeStatus(
  summary: TeamMemberAgentStatusSummary | null,
  runtime?: TeamRuntimeStatusRecord | null
): TeamRuntimeStatusView {
  if (runtime) {
    const total = runtime.members.length;
    const online = runtime.members.filter((member) => member.session_id?.trim()).length;
    if (runtime.status === "running") {
      return {
        status: "running",
        label: "team running",
        tone: "active",
        online,
        total,
      };
    }
    if (runtime.status === "degraded") {
      return {
        status: "degraded",
        label: "team degraded",
        tone: "warning",
        online,
        total,
      };
    }
    return {
      status: "stopped",
      label: "team stopped",
      tone: "inactive",
      online,
      total,
    };
  }
  const online = summary?.active ?? 0;
  const total = summary?.total ?? 0;
  const missing = summary?.missing ?? 0;
  if (total === 0 || online === 0) {
    return {
      status: "stopped",
      label: "team stopped",
      tone: "inactive",
      online,
      total,
    };
  }
  if (online === total && missing === 0) {
    return {
      status: "running",
      label: "team running",
      tone: "active",
      online,
      total,
    };
  }
  return {
    status: "degraded",
    label: "team degraded",
    tone: "warning",
    online,
    total,
  };
}

export function resolveTeamRuntimeControlTone(
  status: TeamRuntimeStatusView["status"]
): TeamRuntimeControlTone {
  if (status === "running") {
    return {
      statusColor: "teal",
      countColor: "teal",
    };
  }
  if (status === "degraded") {
    return {
      statusColor: "yellow",
      countColor: "yellow",
    };
  }
  return {
    statusColor: "gray",
    countColor: "gray",
  };
}

export function resolveTeamPageNotice(message: string | null | undefined): TeamPageNotice | null {
  const normalized = message?.trim() ?? "";
  if (!normalized) {
    return null;
  }
  if (
    normalized.startsWith("Team runtime updated") ||
    normalized.startsWith("Team runtime stopped")
  ) {
    return {
      kind: "runtime",
      title: "Team runtime",
      message: normalized,
    };
  }
  return {
    kind: "warning",
    title: "Team runtime update",
    message: normalized,
  };
}

export function updateCachedTeamRuntimeStatus(
  previousRuntime: TeamRuntimeRecord | undefined,
  teamId: string,
  teamName: string,
  status: TeamRuntimeRecord["status"],
  members: TeamRuntimeControlResponse["members"],
  nextSessionStatus: ((sessionStatus: string | null | undefined) => string | undefined) | null,
  fallbackMemberStatuses?: TeamMemberAgentStatus[]
): TeamRuntimeRecord | undefined {
  const memberUpdates = new Map(
    members.map((member) => [member.member_id, member] as const)
  );
  if (!previousRuntime) {
    if (!fallbackMemberStatuses || fallbackMemberStatuses.length === 0) {
      return undefined;
    }
    const stopped = status === "stopped";
    return {
      team_id: teamId,
      team_name: teamName,
      status,
      members: fallbackMemberStatuses.map((member) => {
        const updated = memberUpdates.get(member.member_id);
        const sessionStatus = stopped
          ? "stopped"
          : nextSessionStatus
            ? nextSessionStatus(member.status)
            : (member.status ?? undefined);
        return {
          member_id: member.member_id,
          display_name: member.agent_name?.trim() || member.member_id,
          role: member.role,
          description: null,
          agent_status: sessionStatus,
          session_id: stopped ? undefined : (updated?.session_id ?? undefined),
          session_status: sessionStatus,
          card: {
            card_id: member.member_id,
            schema_version: "1",
            description: member.role,
            capability_tags: [],
          },
        };
      }),
    };
  }
  const stopped = status === "stopped";
  return {
    ...previousRuntime,
    team_id: teamId,
    team_name: teamName,
    status,
    members: previousRuntime.members.map((member) => {
      const updated = memberUpdates.get(member.member_id);
      return {
        ...member,
        session_id: stopped
          ? undefined
          : updated
            ? (updated.session_id ?? undefined)
            : (member.session_id ?? undefined),
        session_status: stopped
          ? "stopped"
          : nextSessionStatus
            ? nextSessionStatus(member.session_status)
            : (member.session_status ?? undefined),
      };
    }),
  };
}

export function upsertRun(list: TeamRunRecord[], nextRun: TeamRunRecord): TeamRunRecord[] {
  const withoutCurrent = list.filter((run) => run.id !== nextRun.id);
  return sortRuns([nextRun, ...withoutCurrent]);
}

export function upsertEventList(
  prev: TeamRunEventRecord[],
  next: TeamRunEventRecord[],
  mode: "replace" | "prepend"
): TeamRunEventRecord[] {
  const merged = mode === "replace" ? [...next] : [...next, ...prev];
  const byId = new Map<number, TeamRunEventRecord>();
  for (const event of merged) {
    byId.set(event.event_id, event);
  }
  return trimNewestByNumericId(
    [...byId.values()].sort((a, b) => a.event_id - b.event_id),
    mode === "replace" ? TEAM_RUN_EVENT_RETENTION_LIMIT : 0,
    (event) => event.event_id
  );
}

export function upsertAgentEventList(
  prev: AgentEvent[],
  next: AgentEvent[],
  mode: "replace" | "prepend",
  sessionId?: string | null
): AgentEvent[] {
  if (mode === "replace") {
    if (next.length === 0) {
      return [];
    }
    if (sessionId == null) {
      return trimNewestByNumericId(
        [...next].sort((a, b) => a.event_id - b.event_id),
        TEAM_MEMBER_EVENT_RETENTION_LIMIT,
        (event) => event.event_id
      );
    }
    const scopedPrev = prev.filter((event) => (event.session_id ?? null) === sessionId);
    return trimNewestByNumericId(
      mergeOutputsPreserveHistory(scopedPrev, next, true),
      TEAM_MEMBER_EVENT_RETENTION_LIMIT,
      (event) => event.event_id
    );
  }
  const scopedPrev =
    sessionId == null
      ? prev
      : prev.filter((event) => (event.session_id ?? null) === sessionId);
  const merged = [...next, ...scopedPrev];
  const byId = new Map<number, AgentEvent>();
  for (const event of merged) {
    byId.set(event.event_id, event);
  }
  return [...byId.values()].sort((a, b) => a.event_id - b.event_id);
}

function sameConversationMessage(
  left: TeamConversationMessageRecord,
  right: TeamConversationMessageRecord
): boolean {
  return (
    left.message_id === right.message_id &&
    left.conversation_id === right.conversation_id &&
    left.task_id === right.task_id &&
    left.from_actor_id === right.from_actor_id &&
    (left.to_actor_id ?? null) === (right.to_actor_id ?? null) &&
    left.route === right.route &&
    left.created_at === right.created_at
  );
}

export function mergeConversationMessages(
  prev: TeamConversationMessageRecord[],
  next: TeamConversationMessageRecord[]
): TeamConversationMessageRecord[] {
  if (next.length === 0) {
    return prev.length === 0 ? prev : [];
  }
  const prevById = new Map(prev.map((message) => [message.message_id, message] as const));
  let changed = prev.length !== next.length;
  const merged = next.map((message) => {
    const cached = prevById.get(message.message_id);
    if (cached && sameConversationMessage(cached, message)) {
      return cached;
    }
    changed = true;
    return message;
  });
  if (!changed) {
    return prev;
  }
  return trimNewestByNumericId(
    merged,
    TEAM_CONVERSATION_MESSAGE_RETENTION_LIMIT,
    (message) => message.message_id
  );
}

function trimNewestByNumericId<T>(
  items: T[],
  limit: number,
  selectId: (item: T) => number
): T[] {
  if (limit <= 0 || items.length <= limit) {
    return items;
  }
  const startIndex = items.length - limit;
  const trimmed = items.slice(startIndex);
  if (trimmed.length === items.length) {
    return items;
  }
  if (trimmed.length > 1) {
    trimmed.sort((left, right) => selectId(left) - selectId(right));
  }
  return trimmed;
}

export function buildAgentLabel(agent: AgentRecord): string {
  const model = formatAgentModelLabel(agent.command, agent.args) ?? "Unknown";
  return `${agent.name} · ${model} · ${agent.id.slice(0, 8)}`;
}

export function pickNextWorkerAgentId(
  agents: AgentRecord[],
  excludedAgentIds: Set<string>
): string {
  return agents.find((agent) => !excludedAgentIds.has(agent.id))?.id ?? "";
}

export function sortTasksByActivity(tasks: TeamTaskRecord[]): TeamTaskRecord[] {
  return [...tasks].sort((left, right) => {
    if (right.updated_at !== left.updated_at) {
      return right.updated_at - left.updated_at;
    }
    if (right.created_at !== left.created_at) {
      return right.created_at - left.created_at;
    }
    return right.id.localeCompare(left.id);
  });
}

export function isSharedThreadTask(task: TeamTaskRecord): boolean {
  if (task.title.trim().toLowerCase() === DEFAULT_TEAM_THREAD_TITLE) {
    return true;
  }
  if (!task.context || typeof task.context !== "object" || Array.isArray(task.context)) {
    return false;
  }
  return (
    (task.context as { bootstrap_kind?: unknown }).bootstrap_kind ===
    DEFAULT_TEAM_THREAD_BOOTSTRAP_KIND
  );
}

export function resolveTaskChannelId(task: TeamTaskRecord | null | undefined): string | null {
  if (!task?.context || typeof task.context !== "object" || Array.isArray(task.context)) {
    return null;
  }
  const bootstrapKind = (task.context as { bootstrap_kind?: unknown }).bootstrap_kind;
  const channelId = (task.context as { channel_id?: unknown }).channel_id;
  if (bootstrapKind !== TEAM_CHANNEL_TASK_BOOTSTRAP_KIND || typeof channelId !== "string") {
    return null;
  }
  const normalizedChannelId = channelId.trim();
  return normalizedChannelId || null;
}

export function isChannelScopedConversationTask(
  task: TeamTaskRecord | null | undefined,
  channelId: string
): boolean {
  const normalizedChannelId = channelId.trim();
  if (!normalizedChannelId || normalizedChannelId === "all") {
    return false;
  }
  return resolveTaskChannelId(task) === normalizedChannelId;
}

export function listTeamWorkspaceTasks(
  tasks: TeamTaskRecord[],
  teamId: string
): TeamTaskRecord[] {
  return sortTasksByActivity(
    tasks.filter((task) => task.team_id === teamId && !isSharedThreadTask(task))
  );
}

export function resolveSelectedTeamTask(
  tasks: TeamTaskRecord[],
  selectedTaskId: string,
  teamId: string
): TeamTaskRecord | null {
  const teamTasks = listTeamWorkspaceTasks(tasks, teamId);
  const selectedId = selectedTaskId.trim();
  if (selectedId) {
    const selected = teamTasks.find((task) => task.id === selectedId);
    if (selected) {
      return selected;
    }
  }
  return teamTasks[0] ?? null;
}

export function resolveSelectedConversationTask({
  taskList,
  selectedTaskId,
  sharedConversation,
  fallbackTask,
}: {
  taskList: TeamTaskRecord[];
  selectedTaskId: string;
  sharedConversation: TeamTaskRecord | null;
  fallbackTask?: TeamTaskRecord | null;
}): TeamTaskRecord | null {
  const resolvedSelectedTaskId = selectedTaskId.trim();
  if (!resolvedSelectedTaskId) {
    return sharedConversation;
  }
  if (sharedConversation?.id === resolvedSelectedTaskId) {
    return sharedConversation;
  }
  return (
    taskList.find((task) => task.id === resolvedSelectedTaskId) ??
    fallbackTask ??
    null
  );
}

export function resolveChannelLaneConversationTask({
  routeChannelId,
  routeSelectedTaskId,
  selectedConversationTaskId,
  selectedConversation,
  selectedChannelTaskId,
  sharedConversation,
  taskList,
}: {
  routeChannelId: string;
  routeSelectedTaskId?: string | null;
  selectedConversationTaskId?: string | null;
  selectedConversation: TeamTaskRecord | null;
  selectedChannelTaskId?: string | null;
  sharedConversation: TeamTaskRecord | null;
  taskList: TeamTaskRecord[];
}): TeamTaskRecord | null {
  const normalizedChannelId = routeChannelId.trim().toLowerCase();
  const normalizedRouteTaskId = routeSelectedTaskId?.trim() ?? "";
  const normalizedSelectedConversationTaskId = selectedConversationTaskId?.trim() ?? "";
  const selectedConversationChannelId = resolveTaskChannelId(selectedConversation);
  const selectedConversationMatchesExplicitRoute =
    Boolean(normalizedRouteTaskId) && selectedConversation?.id === normalizedRouteTaskId;
  const selectedConversationMatchesLocalSelection =
    Boolean(normalizedSelectedConversationTaskId) &&
    selectedConversation?.id === normalizedSelectedConversationTaskId;
  const shouldPreservePlainTaskSelection =
    selectedConversationMatchesLocalSelection && !selectedConversationChannelId;
  if (!normalizedChannelId) {
    return selectedConversation;
  }
  if (normalizedChannelId === DEFAULT_TEAM_THREAD_TITLE) {
    if (selectedConversationMatchesExplicitRoute || shouldPreservePlainTaskSelection) {
      return selectedConversation;
    }
    return sharedConversation ?? selectedConversation;
  }
  if (
    selectedConversation &&
    (selectedConversationChannelId === normalizedChannelId ||
      shouldPreservePlainTaskSelection ||
      (selectedConversationMatchesExplicitRoute &&
        selectedConversationChannelId !== DEFAULT_TEAM_THREAD_TITLE))
  ) {
    // Channel lanes should keep explicit task conversations, but should not let
    // stale channel-scoped selections from another lane override the lane's
    // canonical conversation.
    return selectedConversation;
  }
  const normalizedSelectedChannelTaskId = selectedChannelTaskId?.trim() ?? "";
  if (!normalizedSelectedChannelTaskId) {
    return selectedConversation;
  }
  // Channel routes should render the lane's canonical conversation even when
  // stale local task selection still points at a previously opened task thread.
  return (
    taskList.find((task) => task.id === normalizedSelectedChannelTaskId) ??
    (selectedConversation?.id === normalizedSelectedChannelTaskId ? selectedConversation : null)
  );
}

function parseMailboxPayload(payload: unknown): unknown {
  if (typeof payload !== "string") {
    return payload;
  }
  const trimmed = payload.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) {
    return payload;
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    return payload;
  }
}

export function resolveTaskMessageSeenByActors(
  mailboxMessages: TeamActorMessageRecord[],
  conversationId: string,
  memberIds: string[]
): Record<number, string[]> {
  const conversationKey = conversationId.trim();
  if (!conversationKey) {
    return {};
  }
  const memberSet = new Set(memberIds.map((memberId) => memberId.trim()).filter(Boolean));
  if (memberSet.size === 0) {
    return {};
  }
  const seenByMessageId = new Map<number, Set<string>>();
  for (const mailboxMessage of mailboxMessages) {
    if (mailboxMessage.status !== "delivered") {
      continue;
    }
    const toActorId = mailboxMessage.to_actor_id.trim();
    if (!memberSet.has(toActorId)) {
      continue;
    }
    const payload = parseMailboxPayload(mailboxMessage.payload);
    if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
      continue;
    }
    const taskConversationId = String(
      (payload as { task_conversation_id?: unknown }).task_conversation_id ?? ""
    ).trim();
    if (taskConversationId !== conversationKey) {
      continue;
    }
    const rawTaskMessageId = (payload as { task_message_id?: unknown }).task_message_id;
    const taskMessageId =
      typeof rawTaskMessageId === "number"
        ? rawTaskMessageId
        : typeof rawTaskMessageId === "string"
          ? Number.parseInt(rawTaskMessageId, 10)
          : Number.NaN;
    if (!Number.isFinite(taskMessageId)) {
      continue;
    }
    const seenActors = seenByMessageId.get(taskMessageId) ?? new Set<string>();
    seenActors.add(toActorId);
    seenByMessageId.set(taskMessageId, seenActors);
  }
  return Object.fromEntries(
    [...seenByMessageId.entries()].map(([messageId, actorIds]) => [
      messageId,
      [...actorIds].sort((left, right) => left.localeCompare(right)),
    ])
  );
}

export function resolveTaskConversationMemberIds(
  runtimeMembers?: Array<Pick<TeamRuntimeRecord["members"][number], "member_id">> | null,
  snapshotMembers?: Array<{ member_id?: string | null }> | null
): string[] {
  const runtimeIds = collectMemberIds(runtimeMembers);
  if (runtimeIds.length > 0) {
    return runtimeIds;
  }
  return collectMemberIds(snapshotMembers);
}

export async function refreshTeamConversationMailboxAfterSend(args: {
  activeRunId?: string | null;
  taskId?: string | null;
  refreshSnapshot: (runId: string) => Promise<unknown>;
  refreshEvents: (runId: string) => Promise<unknown>;
  refreshTaskMessages: (taskIdOverride?: string) => Promise<unknown>;
}): Promise<void> {
  const activeRunId = args.activeRunId?.trim() ?? "";
  if (activeRunId) {
    await Promise.all([
      args.refreshSnapshot(activeRunId),
      args.refreshEvents(activeRunId),
    ]);
    return;
  }
  const taskId = args.taskId?.trim() ?? "";
  if (taskId) {
    await args.refreshTaskMessages(taskId);
  }
}

export function formatTs(ts?: number | null): string {
  if (!ts) return "-";
  return new Date(ts * 1000).toLocaleString();
}

export function toPrettyJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
