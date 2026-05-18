import React, { Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Alert } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { IconButton } from "../ui/primitives";
import {
  AgentDiscoveryCardRecord,
  AgentRecord,
  AgentEvent,
  api,
  getTeamStepRuntimeHandleId,
  TeamConversationMessageRecord,
  TeamActorMessageRecord,
  TeamChannelRecord,
  TeamDefinitionRecord,
  TeamRuntimeRecord,
  TeamTaskDetailResponse,
  TeamTaskRecord,
  TeamTaskRunCompilePreviewRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamRunSnapshotRecord,
  TeamStepRecord,
} from "../api";
import { AGENT_NOT_RUNNING_ERROR, isAgentActiveStatus } from "../agent_ws";
import {
  getNavigatorOnline,
  sanitizeErrorBannerMessage,
  shouldHideErrorBannerMessage,
} from "../connection_status";
import {
  buildTeamDetailPath as buildCanonicalTeamDetailPath,
  buildTeamWorkspacePath as buildCanonicalTeamWorkspacePath,
  buildWorkspaceNodePath,
  isTeamMemberRouteTab,
  navigateToPath,
  resolveTeamMemberRouteTab,
  resolveWorkspaceLens,
  type TeamMemberRouteTab,
  type WorkspaceLens,
} from "../app_route_selection";
import { AuthState } from "../types";
import {
  normalizeWorkdirInput,
  resolveWorkdirForModeChange,
} from "../worktree_defaults";
import {
  type TeamDebugTag,
} from "./team/team_debug_panels";
import { WorkspaceShell } from "../components/layout/workspace_shell";
import { TeamSelectorPanel } from "./team/team_selector_panel";
import {
  prefetchTeamSetupSurface,
  prefetchTeamWorkbenchTab,
} from "./team/team_workbench_content";
import {
  buildTeamPageModalsProps,
} from "./team/team_page_route_props";
import {
  buildTeamChannelItems,
  DEFAULT_TEAM_CHANNEL_ID,
  type TeamChannelId,
  type TeamChannelItem,
} from "./team/channel_metadata";
import { parseErrorMessage } from "./team/create_helpers";
import {
  persistTeamCreateDraft,
} from "./team/create_draft_storage";
import { buildCreateAgentModalProps } from "../components/agents_route_modal_props";
import {
  resolveTeamMemberRoleProfile,
} from "./team/forge_helpers";
import {
  MailboxTemplateKey,
  buildMailboxConversationKey,
  buildMailboxPayloadTemplate,
  countUnreadConversationMessages,
  mergeMailboxMessages,
  resolveConversationMaxMessageId,
  resolveMailboxChatActors,
  selectMailboxConversation,
} from "./team/mailbox_helpers";
import {
  EMPTY_TEAM_PROMPT_DEFAULTS,
  backfillEmptyWorkerDraftPrompts,
  resolveTeamPromptForRole,
} from "./team/member_helpers";
import {
  isChannelScopedConversationTask,
  resolveTaskChannelId,
  resolveTeamPageNotice,
  resolveSelectedAgentWorkspaceMemberId,
  resolveTaskConversationMemberIds,
  resolveTeamMemberAgentControlState,
  shouldClearSelectedConversationTask,
  shouldClearSelectedTeamMember,
  shouldWatchTeamRuntime,
  toPrettyJson,
  upsertRun,
} from "./team/page_helpers";
import {
  selectTeamPreviewEvents,
  type TeamRunStatusFilter,
} from "./team/run_helpers";
import { useTeamActions } from "./team/use_team_actions";
import { useTeamMailboxActions } from "./team/use_team_mailbox_actions";
import { useTeamManagementActions } from "./team/use_team_management_actions";
import { useTeamTaskWorkspaceData } from "./team/use_team_task_workspace_data";
import { useTeamConversationActions } from "./team/use_team_conversation_actions";
import { useTeamConversationEffects } from "./team/use_team_conversation_effects";
import { useTeamMemberAcpEffects } from "./team/use_team_member_acp_effects";
import { useTeamMemberAgentBackfillEffect } from "./team/use_team_member_agent_backfill_effect";
import { useTeamCatalogViewModel } from "./team/use_team_catalog_view_model";
import { useTeamMailboxLifecycleEffects } from "./team/use_team_mailbox_lifecycle_effects";
import { useTeamWorkspaceViewModel } from "./team/use_team_workspace_view_model";
import { useTeamTaskEffects } from "./team/use_team_task_effects";
import { useTeamRuntimeEffects } from "./team/use_team_runtime_effects";
import { useTeamRunLifecycleEffects } from "./team/use_team_run_lifecycle_effects";
import { useTeamStepActions } from "./team/use_team_step_actions";
import { useTeamMemberAcpSessionDiscovery } from "./team/use_team_member_acp_session_discovery";
import { useTeamCachePersistence } from "./team/use_team_cache_persistence";
import { useTeamUiState } from "./team/use_team_ui_state";
import { useTeamControlState } from "./team/use_team_control_state";
import { useTeamCreateState } from "./team/use_team_create_state";
import { useTeamMailboxState } from "./team/use_team_mailbox_state";
import { TeamSidebarContainer } from "./team/TeamSidebarContainer";
import {
  TeamWorkbenchContainer,
  type TeamWorkbenchRuntimeContext,
} from "./team/TeamWorkbenchContainer";
import {
  TeamWorkspaceProvider,
  type TeamWorkspaceContextValue,
} from "./team/team_workspace_context";
import {
  DEFAULT_TEAM_RUN_BROWSER_STATE,
  DEFAULT_WORKTREE_ROOT,
  MAILBOX_TEMPLATE_OPTIONS,
  TEAM_TAB_ITEMS,
  TEAM_RUN_STATUS_FILTER_OPTIONS,
  createInitialTeamCreateState,
  type TeamTab,
  type TeamRunBrowserState,
} from "./team/state";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_SECTION_BODY_TEXT_CLASS,
  TEAM_SOFT_CHROME_SHADOW_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS,
  TEAM_WORKBENCH_PANEL_CLASS,
  TEAM_WORKBENCH_WORKSPACE_SHELL_CLASS,
} from "../ui/tailwind_classes";

function shallowEqualObject<T extends object>(left: T, right: T): boolean {
  const leftRecord = left as Record<string, unknown>;
  const rightRecord = right as Record<string, unknown>;
  const leftKeys = Object.keys(leftRecord);
  const rightKeys = Object.keys(rightRecord);
  return (
    leftKeys.length === rightKeys.length &&
    rightKeys.every(
      (key) =>
        Object.prototype.hasOwnProperty.call(leftRecord, key) &&
        Object.is(leftRecord[key], rightRecord[key])
    )
  );
}

function useShallowStableObject<T extends object>(value: T): T {
  const stableRef = useRef(value);
  if (!shallowEqualObject(stableRef.current, value)) {
    stableRef.current = value;
  }
  return stableRef.current;
}

const loadTeamMemberAcpPanel = () => import("./team_member_acp_panel");
const loadTeamPageModals = () => import("./team/team_page_modals");

const LazyTeamPageModals = React.lazy(async () => {
  const module = await loadTeamPageModals();
  return { default: module.TeamPageModals };
});

export {
  buildMailboxForwardChatPayload,
  buildMailboxChatPayload,
  buildMailboxConversationKey,
  buildMailboxPayloadTemplate,
  countUnreadConversationMessages,
  extractMentionedActorIds,
  mergeMailboxMessages,
  resolveTaskMailboxRoutePlan,
  resolveConversationMaxMessageId,
  resolveMailboxChatActors,
  selectMailboxConversation,
} from "./team/mailbox_helpers";
export {
  DEFAULT_TEAM_COORDINATOR_SKILLS,
  DEFAULT_TEAM_WORKER_SKILLS,
  assignCreatedWorkerToDraft,
  buildDefaultWorkerDraft,
  buildTeamMemberLiveStates,
  createInitialTeamDraftState,
  normalizeSkillSelection,
  parseTeamSpecMembers,
  resolveTeamMemberAgentStatuses,
  resolveTeamMemberLifecycleTone,
  selectTeamForgeAgents,
  summarizeTeamMemberAgentStatuses,
  toggleSkillSelection,
} from "./team/member_helpers";
export {
  mergeRunPages,
  mergeTeamRunList,
  resolveRunStatusFilter,
  selectRunsForTask,
  selectTeamPreviewEvents,
} from "./team/run_helpers";
export {
  removeTeamMemberLookupEntry,
  resolveTeamMemberAgentControlState,
  resolveThreadRootMessageIdFromPayload,
} from "./team/page_helpers";
export type { MailboxTemplateKey, TeamMailboxChatActors } from "./team/mailbox_helpers";
export type {
  TeamMemberAgentStatus,
  TeamMemberAgentStatusSummary,
  TeamMemberLiveState,
  TeamSpecMember,
  WorkerDraft,
} from "./team/member_helpers";

type TeamPageProps = {
  auth: AuthState;
  token: string;
  onLogout: () => void;
  developerMode: boolean;
  routeTeamId: string | null;
  routePathname: string;
  routeSearch?: string;
  defaultWorktreeRoot?: string | null;
};

function toAsciiLowercase(value: string): string {
  return value.replace(/[A-Z]/g, (char) => char.toLowerCase());
}

export function resolveTeamChannelId(search: string): TeamChannelId {
  const params = new URLSearchParams(search);
  const channel = (params.get("channel") ?? "").trim();
  return channel.length === 0 ? DEFAULT_TEAM_CHANNEL_ID : toAsciiLowercase(channel);
}

export function resolveTeamThreadRootMessageId(search: string): number | null {
  const params = new URLSearchParams(search);
  const raw = (params.get("thread") ?? "").trim();
  if (!raw) {
    return null;
  }
  const parsed = Number(raw);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
}

export function resolveTeamSelectedTaskId(search: string): string {
  const params = new URLSearchParams(search);
  return (params.get("task") ?? "").trim();
}

export function resolveRouteScopedConversationTaskSelection(options: {
  previousTaskId: string;
  routeSelectedTaskId: string;
  routeChannelId: TeamChannelId;
  selectedChannelTaskId: string | null | undefined;
}): string | null {
  const {
    previousTaskId,
    routeSelectedTaskId,
    routeChannelId,
    selectedChannelTaskId,
  } = options;
  if (routeSelectedTaskId) {
    return previousTaskId === routeSelectedTaskId ? previousTaskId : routeSelectedTaskId;
  }
  if (routeChannelId === DEFAULT_TEAM_CHANNEL_ID) {
    return previousTaskId ? "" : previousTaskId;
  }
  if (selectedChannelTaskId) {
    return previousTaskId === selectedChannelTaskId ? previousTaskId : selectedChannelTaskId;
  }
  return null;
}

export function resolveChannelRouteTaskId(options: {
  routeSelectedTaskId: string | null | undefined;
  selectedConversationTaskId: string | null | undefined;
  selectedConversationIsShared: boolean;
  selectedConversationMatchesChannelLane: boolean;
  selectedChannelTaskId: string | null | undefined;
}): string | null {
  const {
    routeSelectedTaskId,
    selectedConversationTaskId,
    selectedConversationIsShared,
    selectedConversationMatchesChannelLane,
    selectedChannelTaskId,
  } = options;
  if (!selectedConversationMatchesChannelLane) {
    return null;
  }
  const normalizedChannelTaskId = selectedChannelTaskId?.trim() ?? "";
  const normalizedConversationTaskId = selectedConversationTaskId?.trim() ?? "";
  if (
    !selectedConversationIsShared &&
    normalizedConversationTaskId &&
    normalizedConversationTaskId !== normalizedChannelTaskId
  ) {
    return normalizedConversationTaskId;
  }
  const normalizedRouteTaskId = routeSelectedTaskId?.trim() ?? "";
  // Drop the channel bootstrap task from thread routes so channel-scoped lanes
  // only keep `task=` when the operator is looking at an explicit task conversation.
  if (!normalizedRouteTaskId || normalizedRouteTaskId === normalizedChannelTaskId) {
    return null;
  }
  return normalizedRouteTaskId;
}

export function resolveTeamSelectedMemberId(search: string): string {
  const params = new URLSearchParams(search);
  return (params.get("member") ?? "").trim();
}

export function resolveTeamWorkspaceTab(search: string): TeamTab | null {
  return resolveTeamMemberRouteTab(search);
}

export function parseTeamAgentInputSessionMismatch(
  message: string
): { expected: string; running: string } | null {
  const match = message.match(
    /agent session mismatch:\s*expected=([^\s]+)\s+running=([^\s]+)/
  );
  if (!match) {
    return null;
  }
  const expected = match[1]?.trim();
  const running = match[2]?.trim();
  if (!expected || !running) {
    return null;
  }
  return { expected, running };
}

export function resolveSelectedAgentWorkspaceSessionId(
  latestStep: TeamStepRecord | null | undefined,
  snapshotSessionId: string | null | undefined,
  snapshotSessionStatus: string | null | undefined,
  runtimeSessionId: string | null | undefined,
  previousSessionId?: string | null,
  agentStatus?: string | null,
  runtimeSessionStatus?: string | null,
  runtimeAgentStatus?: string | null
): string | null {
  const normalizedAgentStatus = agentStatus?.trim().toLowerCase() ?? "";
  const normalizedSnapshotSessionStatus = snapshotSessionStatus?.trim().toLowerCase() ?? "";
  const normalizedRuntimeSessionStatus = runtimeSessionStatus?.trim().toLowerCase() ?? "";
  const normalizedRuntimeAgentStatus = runtimeAgentStatus?.trim().toLowerCase() ?? "";
  const normalizedRuntimeSessionId = runtimeSessionId?.trim() ?? "";
  if (normalizedRuntimeSessionId) {
    if (
      normalizedRuntimeSessionStatus &&
      !isAgentActiveStatus(normalizedRuntimeSessionStatus)
    ) {
      return null;
    }
    if (
      !normalizedRuntimeSessionStatus &&
      normalizedRuntimeAgentStatus &&
      !isAgentActiveStatus(normalizedRuntimeAgentStatus)
    ) {
      return null;
    }
    return normalizedRuntimeSessionId;
  }
  const normalizedSnapshotSessionId = snapshotSessionId?.trim() ?? "";
  if (normalizedSnapshotSessionId) {
    if (
      normalizedSnapshotSessionStatus &&
      !isAgentActiveStatus(normalizedSnapshotSessionStatus)
    ) {
      return null;
    }
    return normalizedSnapshotSessionId;
  }
  if (normalizedAgentStatus && !isAgentActiveStatus(normalizedAgentStatus)) {
    return null;
  }
  const normalizedPreviousSessionId = previousSessionId?.trim() ?? "";
  // Keep the previous ACP session identity sticky while runtime metadata catches up
  // so a transient latest_step/runtime refresh does not blank the visible thread.
  if (normalizedPreviousSessionId) {
    return normalizedPreviousSessionId;
  }
  const stepRuntimeSessionId = getTeamStepRuntimeHandleId(latestStep);
  return stepRuntimeSessionId?.trim() || null;
}

export function resolveNextSelectedAgentWorkspaceStickySession(
  previous: { memberId: string; sessionId: string | null },
  memberId: string,
  resolvedSessionId: string | null
): { memberId: string; sessionId: string | null } {
  const normalizedMemberId = memberId.trim();
  const normalizedResolvedSessionId = resolvedSessionId?.trim() || null;
  if (!normalizedMemberId) {
    if (!previous.memberId && previous.sessionId == null) {
      return previous;
    }
    return { memberId: "", sessionId: null };
  }
  if (previous.memberId !== normalizedMemberId) {
    return {
      memberId: normalizedMemberId,
      sessionId: normalizedResolvedSessionId,
    };
  }
  if (!normalizedResolvedSessionId || previous.sessionId === normalizedResolvedSessionId) {
    return previous;
  }
  return {
    memberId: normalizedMemberId,
    sessionId: normalizedResolvedSessionId,
  };
}

export function resolveNextSelectedAgentWorkspaceSessionOverride(
  previous: { memberId: string; sessionId: string | null },
  memberId: string,
  runtimeSessionId: string | null | undefined
): { memberId: string; sessionId: string | null } {
  if (!previous.memberId && previous.sessionId == null) {
    return previous;
  }
  if (previous.memberId !== memberId) {
    return { memberId: "", sessionId: null };
  }
  const normalizedRuntimeSessionId = runtimeSessionId?.trim() || null;
  if (normalizedRuntimeSessionId && normalizedRuntimeSessionId === previous.sessionId) {
    return { memberId: "", sessionId: null };
  }
  return previous;
}

export function buildTeamDetailPath(teamId: string): string {
  return buildCanonicalTeamDetailPath(teamId);
}

function navigateTeamRoute(pathname: string): void {
  if (`${location.pathname}${location.search}` === pathname) {
    return;
  }
  window.history.pushState({}, "", pathname);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

function buildTeamSelectorPath(): string {
  return "/workspace/teams";
}

export function buildTeamWorkspacePath(
  teamId: string,
  lens?: WorkspaceLens | null,
  channelId?: TeamChannelId | null,
  threadRootMessageId?: number | null,
  memberId?: string | null,
  tab?: TeamTab | null,
  taskId?: string | null
): string {
  const normalizedTab: TeamMemberRouteTab | null = isTeamMemberRouteTab(tab) ? tab : null;
  return buildCanonicalTeamWorkspacePath(
    teamId,
    lens,
    channelId,
    threadRootMessageId,
    memberId,
    normalizedTab,
    taskId
  );
}

export function buildTeamLensNavigationPath(
  teamId: string,
  lens: WorkspaceLens,
  channelId?: TeamChannelId | null,
  taskId?: string | null
): string {
  return buildTeamWorkspacePath(
    teamId,
    lens,
    lens === "channels" ? (channelId ?? DEFAULT_TEAM_CHANNEL_ID) : null,
    null,
    null,
    null,
    lens === "channels" ? taskId : null
  );
}

function resolveTeamTabForWorkspaceLens(lens: WorkspaceLens): TeamTab | null {
  switch (lens) {
    case "channels":
      return "conversation";
    case "tasks":
      return "tasks";
    case "members":
      return "overview";
    case "search":
      return null;
    default:
      return "conversation";
  }
}

const TEAM_AGENT_ADVANCED_TABS = new Set<TeamTab>([
  "mailbox",
  "member_console",
  "debug",
]);
const TEAM_AGENT_ADVANCED_TAB_ITEMS = TEAM_TAB_ITEMS.filter((item) =>
  TEAM_AGENT_ADVANCED_TABS.has(item.value)
);
const TEAM_UTILITY_ADVANCED_TABS = new Set<TeamTab>([
  "runs",
  "overview",
  "events",
  "steps",
  "mailbox",
  "debug",
]);
const TEAM_UTILITY_ADVANCED_TAB_ITEMS = TEAM_TAB_ITEMS.filter((item) =>
  TEAM_UTILITY_ADVANCED_TABS.has(item.value)
);

const TEAM_EVENT_PREVIEW_LIMIT = 5;
const HUMAN_MAILBOX_ACTOR_ID = "user";

type RunInputValidation = {
  parsed: unknown | undefined;
  error: string | null;
};

export function formatTeamRuntimeActionSummary(
  action: "start" | "stop" | "force",
  members: ReadonlyArray<{ action: string }>
): string {
  const counts = members.reduce<Record<string, number>>((acc, member) => {
    acc[member.action] = (acc[member.action] ?? 0) + 1;
    return acc;
  }, {});
  const parts = Object.entries(counts)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${value}`);
  const prefix =
    action === "start"
      ? "Team runtime updated"
      : action === "stop"
        ? "Team runtime stopped"
        : "Forced new session";
  return parts.length > 0 ? `${prefix} (${parts.join(", ")})` : prefix;
}

export function validateRunInputJson(raw: string): RunInputValidation {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { parsed: undefined, error: null };
  }
  try {
    return { parsed: JSON.parse(trimmed), error: null };
  } catch (err) {
    const message = err instanceof Error ? err.message : "unknown parse error";
    return { parsed: undefined, error: `Run input must be valid JSON (${message})` };
  }
}

const teamRunMetaItemClassName =
  "rounded-md border border-notion-border bg-notion-sidebar px-2 py-0.5 text-[11px] text-notion-text-muted";
const workspaceToolbarClassName =
  `flex flex-wrap items-center gap-1 rounded-[10px] border border-notion-border/70 bg-notion-sidebar/65 p-0.5 ${TEAM_SOFT_CHROME_SHADOW_CLASS}`;
const workspaceToolbarButtonActiveClassName =
  "inline-flex h-7 items-center gap-1 rounded-[8px] bg-white px-2.5 text-[11px] font-semibold text-notion-text shadow-[0_1px_2px_rgba(15,23,42,0.05)]";
const workspaceToolbarButtonIdleClassName =
  "inline-flex h-7 items-center gap-1 rounded-[8px] px-2.5 text-[11px] font-semibold text-notion-text-muted transition hover:bg-white/70 hover:text-notion-text";
const workspaceNoticeClassName =
  "mt-1 flex flex-wrap items-center justify-between gap-2 px-1";
const workspaceNoticeTextClassName =
  "flex min-w-0 flex-1 items-center gap-1.5 text-[11px] text-notion-text-muted";
const teamRuntimeNoticeClassName =
  "mb-4 flex items-start justify-between gap-3 rounded-lg border border-state-success-border bg-state-success-bg px-4 py-3 text-state-success-text shadow-sm";
const teamRuntimeNoticeTitleClassName =
  "text-[11px] font-bold uppercase tracking-wider text-state-success-text opacity-80";
const teamRuntimeNoticeBodyClassName = "mt-1 text-sm leading-relaxed font-medium";

const teamWorkbenchPanelClassName = TEAM_WORKBENCH_PANEL_CLASS;
const teamWorkbenchAccentButtonClassName =
  "!bg-notion-accent !text-white !border-transparent hover:!bg-notion-accent/90 transition shadow-sm active:!translate-y-px";
const teamWorkbenchMutedButtonClassName =
  "!bg-white !text-notion-text !border-notion-border hover:!bg-notion-hover transition shadow-sm active:!translate-y-px";
const teamWorkbenchHeaderActionButtonClassName = "!shrink-0 !whitespace-nowrap";
const teamWorkbenchBadgeClassName =
  "inline-flex items-center rounded-md border border-notion-border bg-notion-sidebar px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted transition hover:bg-notion-hover";
const teamWorkbenchWorkspaceShellClassName = TEAM_WORKBENCH_WORKSPACE_SHELL_CLASS;
const teamWorkbenchSetupChecklistClassName =
  "overflow-hidden rounded-xl border border-notion-border bg-white shadow-md";
const teamWorkbenchInfoStripGridClassName =
  "grid gap-px bg-notion-border lg:grid-cols-3";
const teamWorkbenchInfoStripItemClassName = TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS;
const teamWorkbenchInfoStripLabelClassName = TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS;
const teamWorkbenchInfoStripValueClassName = TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS;
export function TeamPage(props: TeamPageProps) {
  const routeTeamId = props.routeTeamId?.trim() || null;
  const routeWorkspaceLens = useMemo(
    () => resolveWorkspaceLens(props.routePathname, props.routeSearch ?? ""),
    [props.routePathname, props.routeSearch]
  );
  const routeWorkspaceTab = useMemo(
    () => resolveTeamWorkspaceTab(props.routeSearch ?? ""),
    [props.routeSearch]
  );
  const routeChannelId = useMemo(
    () => resolveTeamChannelId(props.routeSearch ?? ""),
    [props.routeSearch]
  );
  const routeThreadRootMessageId = useMemo(
    () => resolveTeamThreadRootMessageId(props.routeSearch ?? ""),
    [props.routeSearch]
  );
  const routeSelectedMemberId = useMemo(
    () => resolveTeamSelectedMemberId(props.routeSearch ?? ""),
    [props.routeSearch]
  );
  const routeSelectedTaskId = useMemo(
    () => resolveTeamSelectedTaskId(props.routeSearch ?? ""),
    [props.routeSearch]
  );
  const isSelectorRoute = routeTeamId == null;
  const routeDefaultWorktreeRoot = React.useMemo(() => {
    const normalized = normalizeWorkdirInput(props.defaultWorktreeRoot ?? "");
    return normalized || DEFAULT_WORKTREE_ROOT;
  }, [props.defaultWorktreeRoot]);
  const isCompactWorkbench = useMediaQuery("(max-width: 1023px)") ?? false;
  const [error, setError] = useState<string | null>(null);
  const [warning, setWarning] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [teamRuntimeByTeamId, setTeamRuntimeByTeamId] = useState<Record<string, TeamRuntimeRecord>>(
    {}
  );
  const [teamsSidebarCollapsed, setTeamsSidebarCollapsed] = useState(false);
  const [workspaceDetailsOpen, setWorkspaceDetailsOpen] = useState(false);
  const [teamDebugTag, setTeamDebugTag] = useState<TeamDebugTag>("run_ops");
  const mobileRouteTeamIdRef = useRef<string | null>(null);
  const previousCompactWorkbenchRef = useRef<boolean>(isCompactWorkbench ?? false);
  useEffect(() => {
    document.body.classList.add("teams-page");
    return () => {
      document.body.classList.remove("teams-page");
    };
  }, []);
  useEffect(() => {
    const enteredCompactWorkbench =
      isCompactWorkbench && previousCompactWorkbenchRef.current !== isCompactWorkbench;
    previousCompactWorkbenchRef.current = isCompactWorkbench;
    if (!isCompactWorkbench || isSelectorRoute) {
      mobileRouteTeamIdRef.current = routeTeamId;
      return;
    }
    if (enteredCompactWorkbench) {
      mobileRouteTeamIdRef.current = routeTeamId;
      setTeamsSidebarCollapsed(true);
      return;
    }
    if (mobileRouteTeamIdRef.current !== routeTeamId) {
      mobileRouteTeamIdRef.current = routeTeamId;
      setTeamsSidebarCollapsed(true);
    }
  }, [isCompactWorkbench, isSelectorRoute, routeTeamId]);
  const navigateToTeamDetail = useCallback((teamId: string) => {
    navigateTeamRoute(buildTeamDetailPath(teamId));
  }, []);
  const navigateToTeamMemberWorkspace = useCallback(
    (teamId: string, memberId: string, tab: TeamTab) => {
      navigateTeamRoute(
        buildTeamWorkspacePath(teamId, "members", null, null, memberId, tab)
      );
    },
    []
  );
  const navigateToTeamSelector = useCallback(() => {
    navigateTeamRoute(buildTeamSelectorPath());
  }, []);

  const {
    tab,
    runLookupId,
    eventsAutoRefresh,
    setTab,
    setRunLookupId,
    setEventsAutoRefresh,
  } = useTeamUiState();

  useEffect(() => {
    if (routeWorkspaceTab) {
      setTab(routeWorkspaceTab);
      return;
    }
    if (!routeWorkspaceLens) {
      return;
    }
    const nextTab = resolveTeamTabForWorkspaceLens(routeWorkspaceLens);
    if (nextTab) {
      setTab(nextTab);
    }
  }, [routeWorkspaceLens, routeWorkspaceTab, setTab]);
  const channelWorkspaceActive =
    routeWorkspaceLens === "channels" || (routeWorkspaceLens == null && tab === "conversation");
  const {
    runContextId,
    runInput,
    stepKey,
    stepMemberId,
    stepDependsOn,
    stepInput,
    selectedStepId,
    stepAction,
    stepRemoteTaskId,
    stepOutput,
    stepFailText,
    stepInputReason,
    stepInputRequiredPayload,
    stepResumePayload,
    setRunContextId,
    setRunInput,
    setStepKey,
    setStepMemberId,
    setStepDependsOn,
    setStepInput,
    setSelectedStepId,
    setStepAction,
    setStepRemoteTaskId,
    setStepOutput,
    setStepFailText,
    setStepInputReason,
    setStepInputRequiredPayload,
    setStepResumePayload,
  } = useTeamControlState();

  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [teamMemberAgentsById, setTeamMemberAgentsById] = useState<
    Record<string, AgentRecord | null>
  >({});
  const [teams, setTeams] = useState<TeamDefinitionRecord[]>([]);
  const [teamSelectorFilter, setTeamSelectorFilter] = useState("");
  const wasSelectorRouteRef = useRef(isSelectorRoute);
  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(routeTeamId);
  const effectiveSelectedTeamId = isSelectorRoute ? null : routeTeamId ?? selectedTeamId;
  const sharedConversationRequestScopeRef = useRef({
    teamId: routeTeamId ?? "",
    requestSeq: 0,
  });
  const setRouteScopedSelectedTeamId = useCallback<React.Dispatch<React.SetStateAction<string | null>>>(
    (next) => {
      setSelectedTeamId((current) => {
        const resolved = typeof next === "function" ? next(current) : next;
        if (isSelectorRoute) {
          return null;
        }
        if (routeTeamId && resolved && resolved !== routeTeamId) {
          return routeTeamId;
        }
        return resolved;
      });
    },
    [isSelectorRoute, routeTeamId, setSelectedTeamId]
  );
  useEffect(() => {
    if (isSelectorRoute && !wasSelectorRouteRef.current) {
      setTeamSelectorFilter("");
    }
    wasSelectorRouteRef.current = isSelectorRoute;
  }, [isSelectorRoute]);

  const {
    teamCreateState,
    coordinatorPrompt,
    workers,
    newTeamName,
    newTeamDescription,
    showCreateTeamModal,
    showForgeAgentForm,
    showCopyExistingAgentModal,
    forgeAgentName,
    forgeAgentWorkdir,
    forgeAgentPresetId,
    forgeAgentCodexAcpDefaultMode,
    forgeAgentWorktreeMode,
    forgeAgentWorktreeRepo,
    forgeAgentWorktreeRef,
    forgeAgentCodeMode,
    forgeAgentWorktreeError,
    forgeAgentBusy,
    forgeDefaultWorktreeRoot,
    setForgeDefaultWorktreeRoot,
    teamPromptDefaults,
    setTeamPromptDefaults,
    teamMemberDraft,
    setTeamMemberDraft,
    teamMemberEditDraft,
    setTeamMemberEditDraft,
    showTeamMemberEditModal,
    setShowTeamMemberEditModal,
    patchTeamCreate,
    setNewTeamName,
    setNewTeamDescription,
    setShowCreateTeamModal,
    setShowForgeAgentForm,
    setShowCopyExistingAgentModal,
    setForgeAgentName,
    setForgeAgentWorkdir,
    patchTeamMemberDraft,
    patchTeamMemberEditDraft,
    setForgeAgentPresetId,
    setForgeAgentCodexAcpDefaultMode,
    setForgeAgentWorktreeMode,
    setForgeAgentWorktreeRepo,
    setForgeAgentWorktreeRef,
    setForgeAgentCodeMode,
    setForgeAgentWorktreeError,
    setForgeAgentBusy,
  } = useTeamCreateState();

  const createDraftPersistErrorRef = useRef<string | null>(null);

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
  const [memberDiscoveryCardsById, setMemberDiscoveryCardsById] = useState<
    Record<string, AgentDiscoveryCardRecord | null>
  >({});
  const [memberDiscoveryCardLoadingById, setMemberDiscoveryCardLoadingById] = useState<
    Record<string, boolean>
  >({});
  const [teamChannels, setTeamChannels] = useState<TeamChannelRecord[]>([]);
  const [teamChannelsSettled, setTeamChannelsSettled] = useState(false);
  const [teamChannelsLoadedSuccessfully, setTeamChannelsLoadedSuccessfully] = useState(false);
  const [deletingChannelId, setDeletingChannelId] = useState<string | null>(null);
  const [taskList, setTaskList] = useState<TeamTaskRecord[]>([]);
  const channelItems = useMemo<ReadonlyArray<TeamChannelItem>>(
    () => buildTeamChannelItems(teamChannels),
    [teamChannels]
  );
  const selectedChannelItem = useMemo(
    () => channelItems.find((item) => item.id === routeChannelId) ?? channelItems[0],
    [channelItems, routeChannelId]
  );
  const selectedChannelRecord = useMemo(
    () => teamChannels.find((channel) => channel.channel_id === routeChannelId) ?? null,
    [routeChannelId, teamChannels]
  );
  const [sharedConversation, setSharedConversation] = useState<TeamTaskRecord | null>(null);
  const [sharedConversationLatestRun, setSharedConversationLatestRun] =
    useState<TeamRunRecord | null>(null);
  const [selectedConversationTaskId, setSelectedConversationTaskId] = useState("");
  const [selectedConversationDetail, setSelectedConversationDetail] =
    useState<TeamTaskDetailResponse | null>(null);
  const [tasksLoading, setTasksLoading] = useState(false);
  const [selectedTaskId, setSelectedTaskId] = useState("");
  const [taskMessages, setTaskMessages] = useState<TeamConversationMessageRecord[]>([]);
  const [conversationMailboxMessages, setConversationMailboxMessages] = useState<
    TeamActorMessageRecord[]
  >([]);
  const [taskMessagesLoading, setTaskMessagesLoading] = useState(false);
  const [taskMessageDraft, setTaskMessageDraft] = useState("");
  const [threadReplyDraft, setThreadReplyDraft] = useState("");
  const [compilePreviewContextId, setCompilePreviewContextId] = useState("");
  const [compiledRunPreview, setCompiledRunPreview] =
    useState<TeamTaskRunCompilePreviewRecord | null>(null);

  const [events, setEvents] = useState<TeamRunEventRecord[]>([]);
  const [eventsHasMore, setEventsHasMore] = useState(false);
  const [eventsLoading, setEventsLoading] = useState(false);

  const [steps, setSteps] = useState<TeamStepRecord[]>([]);

  const {
    msgFromActorId,
    msgToActorId,
    msgChannel,
    msgTransport,
    msgRoute,
    msgTemplate,
    msgPayload,
    msgIdempotencyKey,
    chatDraft,
    chatStickToBottom,
    chatSeenByConversation,
    inboxActorId,
    inboxLimit,
    inboxAfterId,
    inboxIncludeDelivered,
    inbox,
    setInbox,
    selectedMemberId,
    setMsgFromActorId,
    setMsgToActorId,
    setMsgChannel,
    setMsgTransport,
    setMsgRoute,
    setMsgTemplate,
    setMsgPayload,
    setMsgIdempotencyKey,
    setChatDraft,
    setChatStickToBottom,
    setChatSeenByConversation,
    setInboxActorId,
    setInboxLimit,
    setInboxAfterId,
    setInboxIncludeDelivered,
    setSelectedMemberId,
    markConversationSeen,
  } = useTeamMailboxState();

  const chatMessagesRef = useRef<HTMLUListElement | null>(null);

  const eventsRef = useRef<TeamRunEventRecord[]>([]);
  const [memberEvents, setMemberEvents] = useState<AgentEvent[]>([]);
  const [memberEventsHasMore, setMemberEventsHasMore] = useState(false);
  const [memberEventsLoading, setMemberEventsLoading] = useState(false);
  const memberEventsRef = useRef<AgentEvent[]>([]);
  const [focusedAgentMemberId, setFocusedAgentMemberId] = useState("");
  const [teamCatalogSettled, setTeamCatalogSettled] = useState(false);
  const handleTeamsRefreshSettled = useCallback(() => {
    setTeamCatalogSettled(true);
  }, []);

  const selectedTeam = useMemo(
    () => teams.find((team) => team.id === effectiveSelectedTeamId) ?? null,
    [effectiveSelectedTeamId, teams]
  );
  useEffect(() => {
    if (teams.length > 0) {
      setTeamCatalogSettled(true);
    }
  }, [teams.length]);
  useEffect(() => {
    setSelectedTeamId(routeTeamId);
  }, [routeTeamId]);
  useEffect(() => {
    if (routeTeamId == null) {
      return;
    }
    if (teams.length === 0) {
      setTeamCatalogSettled(false);
    }
  }, [routeTeamId, teams.length]);
  useEffect(() => {
    sharedConversationRequestScopeRef.current = {
      teamId: effectiveSelectedTeamId?.trim() ?? "",
      requestSeq: sharedConversationRequestScopeRef.current.requestSeq + 1,
    };
  }, [effectiveSelectedTeamId]);
  useEffect(() => {
    setCompiledRunPreview(null);
    setCompilePreviewContextId("");
    setTeamChannels([]);
    setTeamChannelsSettled(false);
    setDeletingChannelId(null);
    setTaskList([]);
    setSharedConversation(null);
    setSharedConversationLatestRun(null);
    setSelectedConversationTaskId("");
    setSelectedConversationDetail(null);
    setTasksLoading(false);
    setSelectedTaskId("");
    setTaskMessages([]);
    setConversationMailboxMessages([]);
    setTaskMessagesLoading(false);
    setTaskMessageDraft("");
    setSelectedMemberId("");
    setFocusedAgentMemberId("");
  }, [effectiveSelectedTeamId, setSelectedMemberId]);
  const refreshTeamChannels = useCallback(
    async (teamId: string) => {
      const normalizedTeamId = teamId.trim();
      if (!normalizedTeamId) {
        return [] as TeamChannelRecord[];
      }
      setTeamChannelsSettled(false);
      setTeamChannelsLoadedSuccessfully(false);
      try {
        const channels = await api.listTeamChannels(props.token, normalizedTeamId);
        setTeamChannels(channels);
        setTeamChannelsLoadedSuccessfully(true);
        return channels;
      } catch (err) {
        setError(parseErrorMessage(err));
        return [];
      } finally {
        setTeamChannelsSettled(true);
      }
    },
    [props.token]
  );
  useEffect(() => {
    if (!effectiveSelectedTeamId) {
      return;
    }
    let active = true;
    setTeamChannelsSettled(false);
    setTeamChannelsLoadedSuccessfully(false);
    void api
      .listTeamChannels(props.token, effectiveSelectedTeamId)
      .then((channels) => {
        if (!active) {
          return;
        }
        setTeamChannels(channels);
        setTeamChannelsLoadedSuccessfully(true);
      })
      .catch((err) => {
        if (!active) {
          return;
        }
        setError(parseErrorMessage(err));
        setTeamChannelsLoadedSuccessfully(false);
      })
      .finally(() => {
        if (active) {
          setTeamChannelsSettled(true);
        }
      });
    return () => {
      active = false;
    };
  }, [effectiveSelectedTeamId, props.token]);
  useEffect(() => {
    const routeTargetsChannelLane =
      routeWorkspaceLens === "channels" || routeChannelId !== DEFAULT_TEAM_CHANNEL_ID;
    if (!effectiveSelectedTeamId || !routeTargetsChannelLane) {
      return;
    }
    const selectedTaskId = resolveRouteScopedConversationTaskSelection({
      previousTaskId: selectedConversationTaskId,
      routeSelectedTaskId,
      routeChannelId,
      selectedChannelTaskId: selectedChannelRecord?.task_id,
    });
    if (selectedTaskId !== null) {
      setSelectedConversationTaskId(() => selectedTaskId);
      return;
    }
    if (teamChannelsSettled && teamChannelsLoadedSuccessfully) {
      navigateTeamRoute(buildTeamWorkspacePath(effectiveSelectedTeamId, "channels"));
    }
  }, [
    effectiveSelectedTeamId,
    routeChannelId,
    routeSelectedTaskId,
    routeWorkspaceLens,
    selectedConversationTaskId,
    selectedChannelRecord,
    teamChannelsLoadedSuccessfully,
    teamChannelsSettled,
  ]);
  useEffect(() => {
    const memberId = routeSelectedMemberId.trim();
    if (!memberId) {
      return;
    }
    setSelectedMemberId(memberId);
    setFocusedAgentMemberId(memberId);
  }, [routeSelectedMemberId, setSelectedMemberId]);
  const {
    teamSpecMemberIds,
    teamMemberSummaryByTeamId,
    selectorTeamItems,
    selectedTeamMemberStatuses,
    selectedTeamMemberLiveStates,
    selectedTeamMemberSummary,
    selectedTeamRuntime,
    selectedTeamRuntimeStatus,
    selectedTeamRuntimeControlTone,
    selectedTeamMembers,
    selectedTeamHasConfiguredMembers,
    selectedTeamHasCoordinator,
    selectedTeamWorkerCount,
  } = useTeamCatalogViewModel({
    teams,
    agents,
    teamMemberAgentsById,
    teamRuntimeByTeamId,
    selectedTeam,
    snapshot,
    teamSelectorFilter,
  });
  useTeamMemberAgentBackfillEffect({
    token: props.token,
    agents,
    teamSpecMemberIds,
    teamMemberAgentsById,
    setTeamMemberAgentsById,
  });
  const shouldWatchSelectedTeamRuntime = useMemo(
    () =>
      shouldWatchTeamRuntime({
        selectedTeamHasConfiguredMembers,
      }),
    [selectedTeamHasConfiguredMembers]
  );
  const teamMemberRoleProfile = useMemo(
    () => (teamMemberDraft ? resolveTeamMemberRoleProfile(teamMemberDraft.role) : null),
    [teamMemberDraft]
  );
  const teamExecutionBlockedReason = useMemo(() => {
    if (!selectedTeam) {
      return null;
    }
    if (!selectedTeamHasConfiguredMembers) {
      return "Add at least one agent before starting the team runtime or a run.";
    }
    return null;
  }, [selectedTeam, selectedTeamHasConfiguredMembers]);
  const teamMemberForgeLabel = "Create New Agent";
  const teamMemberCopyExistingLabel = "Copy Existing Agent";
  const copyExistingTeamAgentCandidates = useMemo(() => {
    const selectedMemberIds = new Set(selectedTeamMembers.map((member) => member.member_id));
    return agents
      .filter((agent) => !selectedMemberIds.has(agent.id))
      .sort((left, right) => left.name.localeCompare(right.name));
  }, [agents, selectedTeamMembers]);
  useEffect(() => {
    const memberId = selectedMemberId.trim();
    if (!memberId) {
      return;
    }
    if (
      shouldClearSelectedTeamMember({
        selectedMemberId: memberId,
        memberIds: selectedTeamMemberLiveStates.map((member) => member.member_id),
      })
    ) {
      setSelectedMemberId("");
    }
  }, [selectedMemberId, selectedTeamMemberLiveStates, setSelectedMemberId]);
  useEffect(() => {
    const memberId = focusedAgentMemberId.trim();
    if (!memberId) {
      return;
    }
    if (
      shouldClearSelectedTeamMember({
        selectedMemberId: memberId,
        memberIds: selectedTeamMemberLiveStates.map((member) => member.member_id),
      })
    ) {
      setFocusedAgentMemberId("");
    }
  }, [focusedAgentMemberId, selectedTeamMemberLiveStates]);
  useEffect(() => {
    if (tab !== "mailbox" || !snapshot || selectedMemberId.trim()) {
      return;
    }
    const defaultMailboxMemberId =
      snapshot.members.find((member) => member.member_id !== snapshot.coordinator_member_id)
        ?.member_id ??
      snapshot.members[0]?.member_id ??
      "";
    if (defaultMailboxMemberId) {
      setSelectedMemberId(defaultMailboxMemberId);
    }
  }, [selectedMemberId, setSelectedMemberId, snapshot, tab]);

  const activeRun = useMemo(
    () => runs.find((run) => run.id === activeRunId) ?? null,
    [runs, activeRunId]
  );
  const activeRunForSelectedTeam = useMemo(() => {
    if (!activeRun || !effectiveSelectedTeamId) {
      return null;
    }
    if (activeRun.team_id !== effectiveSelectedTeamId) {
      return null;
    }
    return activeRun;
  }, [activeRun, effectiveSelectedTeamId]);
  const activeRunIdForSelectedTeam = activeRunForSelectedTeam?.id ?? null;
  const canResumeActiveRun = useMemo(() => {
    if (!activeRunForSelectedTeam) return false;
    return (
      activeRunForSelectedTeam.status === "failed" ||
      activeRunForSelectedTeam.status === "canceled"
    );
  }, [activeRunForSelectedTeam]);
  const canRestartActiveRun = useMemo(() => {
    if (!activeRunForSelectedTeam) return false;
    return (
      activeRunForSelectedTeam.status === "failed" ||
      activeRunForSelectedTeam.status === "canceled" ||
      activeRunForSelectedTeam.status === "completed"
    );
  }, [activeRunForSelectedTeam]);
  const selectedTeamRunBrowserState = useMemo<TeamRunBrowserState>(() => {
    if (!effectiveSelectedTeamId) {
      return DEFAULT_TEAM_RUN_BROWSER_STATE;
    }
    return teamRunBrowserByTeam[effectiveSelectedTeamId] ?? DEFAULT_TEAM_RUN_BROWSER_STATE;
  }, [effectiveSelectedTeamId, teamRunBrowserByTeam]);
  const runStatusFilter = selectedTeamRunBrowserState.statusFilter;
  const runsHasMore = selectedTeamRunBrowserState.hasMore;
  const runsBeforeCreatedAt = selectedTeamRunBrowserState.beforeCreatedAt;
  const totalLoadedRunsForTeam = useMemo(() => {
    if (!effectiveSelectedTeamId) return 0;
    return runs.filter((run) => run.team_id === effectiveSelectedTeamId).length;
  }, [effectiveSelectedTeamId, runs]);

  const visibleRuns = useMemo(() => {
    if (!effectiveSelectedTeamId) return [];
    return runs.filter((run) => {
      if (run.team_id !== effectiveSelectedTeamId) return false;
      if (runStatusFilter === "all") return true;
      return run.status === runStatusFilter;
    });
  }, [effectiveSelectedTeamId, runStatusFilter, runs]);
  const isActiveRunHiddenByFilter = useMemo(() => {
    if (!activeRunForSelectedTeam || !effectiveSelectedTeamId) return false;
    if (runStatusFilter === "all") return false;
    return activeRunForSelectedTeam.status !== runStatusFilter;
  }, [activeRunForSelectedTeam, effectiveSelectedTeamId, runStatusFilter]);

  const selectedMemberSnapshot = useMemo(
    () => snapshot?.members.find((member) => member.member_id === selectedMemberId) ?? null,
    [selectedMemberId, snapshot]
  );
  const knownSelectedTeamMemberIds = useMemo(
    () =>
      Array.from(
        new Set([
          ...selectedTeamMemberLiveStates.map((member) => member.member_id),
          ...(snapshot?.members.map((member) => member.member_id) ?? []),
          ...selectedTeamMembers.map((member) => member.member_id),
        ].map((memberId) => memberId.trim()).filter(Boolean))
      ),
    [selectedTeamMemberLiveStates, selectedTeamMembers, snapshot]
  );
  const selectedAgentWorkspaceMemberId = useMemo(
    () =>
      resolveSelectedAgentWorkspaceMemberId({
        selectedMemberId,
        focusedAgentMemberId,
        routeSelectedMemberId,
        knownMemberIds: knownSelectedTeamMemberIds,
      }),
    [focusedAgentMemberId, knownSelectedTeamMemberIds, routeSelectedMemberId, selectedMemberId]
  );
  const selectedAgentWorkspaceSnapshot = useMemo(
    () =>
      snapshot?.members.find((member) => member.member_id === selectedAgentWorkspaceMemberId) ??
      null,
    [selectedAgentWorkspaceMemberId, snapshot]
  );
  const selectedAgentWorkspaceRuntimeMember = useMemo(() => {
    if (!selectedAgentWorkspaceMemberId) {
      return null;
    }
    return (
      selectedTeamRuntime?.members.find(
        (member) => member.member_id === selectedAgentWorkspaceMemberId
      ) ?? null
    );
  }, [selectedAgentWorkspaceMemberId, selectedTeamRuntime]);
  const selectedAgentWorkspaceAgent = useMemo(() => {
    const memberId = selectedAgentWorkspaceMemberId.trim();
    if (!memberId) {
      return null;
    }
    return teamMemberAgentsById[memberId] ?? agents.find((agent) => agent.id === memberId) ?? null;
  }, [agents, selectedAgentWorkspaceMemberId, teamMemberAgentsById]);
  const [selectedAgentWorkspaceStickySession, setSelectedAgentWorkspaceStickySession] = useState<{
    memberId: string;
    sessionId: string | null;
  }>({ memberId: "", sessionId: null });
  const [selectedAgentWorkspaceSessionOverride, setSelectedAgentWorkspaceSessionOverride] =
    useState<{
      memberId: string;
      sessionId: string | null;
    }>({ memberId: "", sessionId: null });
  const selectedAgentWorkspaceResolvedSessionId = useMemo(() => {
    const overrideSessionId =
      selectedAgentWorkspaceSessionOverride.memberId === selectedAgentWorkspaceMemberId
        ? selectedAgentWorkspaceSessionOverride.sessionId
        : null;
    if (overrideSessionId?.trim()) {
      return overrideSessionId.trim();
    }
    const previousSessionId =
      selectedAgentWorkspaceStickySession.memberId === selectedAgentWorkspaceMemberId
        ? selectedAgentWorkspaceStickySession.sessionId
        : null;
    return resolveSelectedAgentWorkspaceSessionId(
      selectedAgentWorkspaceSnapshot?.latest_step,
      selectedAgentWorkspaceSnapshot?.session_id ?? null,
      selectedAgentWorkspaceSnapshot?.session_status ?? null,
      selectedAgentWorkspaceRuntimeMember?.session_id ?? null,
      previousSessionId,
      selectedAgentWorkspaceAgent?.status ?? null,
      selectedAgentWorkspaceRuntimeMember?.session_status ?? null,
      selectedAgentWorkspaceRuntimeMember?.agent_status ?? null
    );
  }, [
    selectedAgentWorkspaceAgent,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceRuntimeMember,
    selectedAgentWorkspaceSessionOverride,
    selectedAgentWorkspaceSnapshot,
    selectedAgentWorkspaceStickySession,
  ]);
  const selectedAgentWorkspaceSessionId = selectedAgentWorkspaceResolvedSessionId;
  useEffect(() => {
    setSelectedAgentWorkspaceSessionOverride((previous) => {
      return resolveNextSelectedAgentWorkspaceSessionOverride(
        previous,
        selectedAgentWorkspaceMemberId,
        selectedAgentWorkspaceRuntimeMember?.session_id ?? null
      );
    });
  }, [selectedAgentWorkspaceMemberId, selectedAgentWorkspaceRuntimeMember]);
  useEffect(() => {
    setSelectedAgentWorkspaceStickySession((previous) =>
      resolveNextSelectedAgentWorkspaceStickySession(
        previous,
        selectedAgentWorkspaceMemberId,
        selectedAgentWorkspaceResolvedSessionId
      )
    );
  }, [selectedAgentWorkspaceMemberId, selectedAgentWorkspaceResolvedSessionId]);
  const memberTargetNodeById = useMemo<Record<string, string | null>>(() => {
    const entries = Object.entries(teamMemberAgentsById).map(([memberId, agent]) => [
      memberId,
      agent ? agent.target_node_id?.trim() || "main" : null,
    ]);
    return Object.fromEntries(entries);
  }, [teamMemberAgentsById]);
  const focusedMemberTargetNodeId = useMemo(() => {
    const memberId = focusedAgentMemberId.trim();
    if (!memberId) {
      return null;
    }
    return memberTargetNodeById[memberId]?.trim() || null;
  }, [focusedAgentMemberId, memberTargetNodeById]);
  const selectedAgentWorkspaceAgentId = selectedAgentWorkspaceAgent?.id?.trim() ?? "";
  const selectedAgentWorkspaceEventAgentId =
    selectedAgentWorkspaceAgentId || selectedAgentWorkspaceMemberId.trim();
  const selectedMemberDiscoveryCard = useMemo(() => {
    const memberId = selectedMemberId.trim();
    if (!memberId) return null;
    return memberDiscoveryCardsById[memberId] ?? null;
  }, [memberDiscoveryCardsById, selectedMemberId]);
  const selectedMemberDiscoveryCardLoading = useMemo(() => {
    const memberId = selectedMemberId.trim();
    if (!memberId) return false;
    return memberDiscoveryCardLoadingById[memberId] ?? false;
  }, [memberDiscoveryCardLoadingById, selectedMemberId]);
  useEffect(() => {
    const memberId = selectedMemberId.trim();
    if (!props.token || !memberId) {
      return;
    }
    if (Object.prototype.hasOwnProperty.call(memberDiscoveryCardsById, memberId)) {
      return;
    }

    let active = true;
    setMemberDiscoveryCardLoadingById((prev) => ({ ...prev, [memberId]: true }));
    void api
      .getAgentDiscoveryCard(props.token, memberId)
      .then((card) => {
        if (!active) return;
        setMemberDiscoveryCardsById((prev) => ({ ...prev, [memberId]: card }));
      })
      .catch(() => {
        if (!active) return;
        setMemberDiscoveryCardsById((prev) => ({ ...prev, [memberId]: null }));
      })
      .finally(() => {
        if (!active) return;
        setMemberDiscoveryCardLoadingById((prev) => ({ ...prev, [memberId]: false }));
      });

    return () => {
      active = false;
    };
  }, [
    memberDiscoveryCardsById,
    props.token,
    selectedMemberId,
  ]);
  const chatMemberIds = useMemo(() => {
    const memberIds = snapshot?.members.map((member) => member.member_id) ?? [];
    if (!memberIds.includes(HUMAN_MAILBOX_ACTOR_ID)) {
      memberIds.push(HUMAN_MAILBOX_ACTOR_ID);
    }
    return memberIds;
  }, [snapshot]);
  const taskConversationMemberIds = useMemo(
    () =>
      resolveTaskConversationMemberIds(
        selectedTeamRuntime?.members,
        snapshot?.members
      ),
    [selectedTeamRuntime?.members, snapshot?.members]
  );
  const chatActors = useMemo(
    () =>
      resolveMailboxChatActors(
        snapshot?.coordinator_member_id,
        chatMemberIds,
        selectedMemberId
      ),
    [chatMemberIds, selectedMemberId, snapshot?.coordinator_member_id]
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
    for (const actorId of chatMemberIds) {
      const actors = resolveMailboxChatActors(
        snapshot.coordinator_member_id,
        chatMemberIds,
        actorId
      );
      const key = buildMailboxConversationKey(actors.fromActorId, actors.toActorId);
      const seenMessageId = key ? chatSeenByConversation[key] ?? 0 : 0;
      counts[actorId] = countUnreadConversationMessages(
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
  const oldestEventId = events.length > 0 ? events[0].event_id : null;
  const oldestMemberEventId =
    memberEvents.length > 0 ? memberEvents[0].event_id : null;

  const resetTeamDraft = useCallback(() => {
    const initial = createInitialTeamCreateState();
    patchTeamCreate({
      newTeamName: initial.newTeamName,
      newTeamDescription: initial.newTeamDescription,
      coordinatorPrompt: teamPromptDefaults.coordinator_prompt,
      showForgeAgentForm: initial.showForgeAgentForm,
      showCopyExistingAgentModal: initial.showCopyExistingAgentModal,
      forgeAgentName: initial.forgeAgentName,
      forgeAgentWorkdir: initial.forgeAgentWorkdir,
      forgeAgentPresetId: initial.forgeAgentPresetId,
      forgeAgentCodexAcpDefaultMode: initial.forgeAgentCodexAcpDefaultMode,
      forgeAgentWorktreeMode: initial.forgeAgentWorktreeMode,
      forgeAgentWorktreeRepo: initial.forgeAgentWorktreeRepo,
      forgeAgentWorktreeRef: initial.forgeAgentWorktreeRef,
      forgeAgentCodeMode: initial.forgeAgentCodeMode,
      forgeAgentWorktreeError: initial.forgeAgentWorktreeError,
      forgeAgentBusy: initial.forgeAgentBusy,
    });
    setTeamMemberDraft(null);
  }, [patchTeamCreate, teamPromptDefaults.coordinator_prompt]);

  useEffect(() => {
    if (!props.token) {
      setForgeDefaultWorktreeRoot(DEFAULT_WORKTREE_ROOT);
      return;
    }
    if (props.defaultWorktreeRoot != null) {
      setForgeDefaultWorktreeRoot(routeDefaultWorktreeRoot);
      return;
    }
    let active = true;
    void api
      .getRuntimeDefaults(props.token)
      .then((runtimeDefaults) => {
        if (!active) {
          return;
        }
        const root = normalizeWorkdirInput(runtimeDefaults.default_worktree_root);
        setForgeDefaultWorktreeRoot(root || DEFAULT_WORKTREE_ROOT);
      })
      .catch((err) => {
        if (!active || (!showCreateTeamModal && !showForgeAgentForm)) {
          return;
        }
        setError(`Failed to load Team runtime defaults: ${parseErrorMessage(err)}`);
      });
    return () => {
      active = false;
    };
  }, [
    props.defaultWorktreeRoot,
    props.token,
    routeDefaultWorktreeRoot,
    setError,
    showCreateTeamModal,
    showForgeAgentForm,
  ]);

  useEffect(() => {
    if (!props.token) {
      setTeamPromptDefaults(EMPTY_TEAM_PROMPT_DEFAULTS);
      return;
    }
    let active = true;
    void api
      .getTeamPromptDefaults(props.token)
      .then((promptDefaults) => {
        if (!active) {
          return;
        }
        setTeamPromptDefaults(promptDefaults);
      })
      .catch((err) => {
        if (!active || (!showCreateTeamModal && !showForgeAgentForm)) {
          return;
        }
        setError(`Failed to load Team defaults: ${parseErrorMessage(err)}`);
      });
    return () => {
      active = false;
    };
  }, [props.token, setError, showCreateTeamModal, showForgeAgentForm]);

  useEffect(() => {
    if (!showCreateTeamModal || coordinatorPrompt.trim()) {
      return;
    }
    if (!teamPromptDefaults.coordinator_prompt.trim()) {
      return;
    }
    patchTeamCreate({ coordinatorPrompt: teamPromptDefaults.coordinator_prompt });
  }, [
    patchTeamCreate,
    showCreateTeamModal,
    coordinatorPrompt,
    teamPromptDefaults.coordinator_prompt,
  ]);

  useEffect(() => {
    if (!showCreateTeamModal) {
      return;
    }
    const nextWorkers = backfillEmptyWorkerDraftPrompts(
      workers,
      teamPromptDefaults
    );
    if (nextWorkers === workers) {
      return;
    }
    patchTeamCreate({ workers: nextWorkers });
  }, [
    patchTeamCreate,
    showCreateTeamModal,
    workers,
    teamPromptDefaults,
  ]);

  useEffect(() => {
    if (!teamMemberDraft || teamMemberDraft.prompt.trim()) {
      return;
    }
    const prompt = resolveTeamPromptForRole(teamPromptDefaults, teamMemberDraft.role);
    if (!prompt.trim()) {
      return;
    }
    patchTeamMemberDraft({ prompt });
  }, [patchTeamMemberDraft, teamMemberDraft, teamPromptDefaults]);

  useEffect(() => {
    if (!teamMemberEditDraft || teamMemberEditDraft.prompt.trim()) {
      return;
    }
    const prompt = resolveTeamPromptForRole(teamPromptDefaults, teamMemberEditDraft.role);
    if (!prompt.trim()) {
      return;
    }
    patchTeamMemberEditDraft({ prompt });
  }, [patchTeamMemberEditDraft, teamMemberEditDraft, teamPromptDefaults]);

  useEffect(() => {
    if (!showCreateTeamModal || busy === "create-team") {
      return;
    }
    const persistErr = persistTeamCreateDraft(teamCreateState);
    if (persistErr) {
      if (persistErr !== createDraftPersistErrorRef.current) {
        createDraftPersistErrorRef.current = persistErr;
        setError(persistErr);
      }
      return;
    }
    createDraftPersistErrorRef.current = null;
  }, [busy, showCreateTeamModal, teamCreateState, setError]);

  useEffect(() => {
    eventsRef.current = events;
  }, [events]);

  useEffect(() => {
    activeRunIdRef.current = activeRunId;
  }, [activeRunId]);

  useEffect(() => {
    memberEventsRef.current = memberEvents;
  }, [memberEvents]);

  const applyCreatedRunState = useCallback(
    (created: TeamRunRecord, syncRunEditor: boolean) => {
      setRuns((prev) => upsertRun(prev, created));
      setActiveRunId(created.id);
      setRunLookupId(created.id);
      if (syncRunEditor) {
        setRunContextId(created.context_id);
        setRunInput(toPrettyJson(created.input));
      }
    },
    [setActiveRunId, setRunContextId, setRunInput, setRunLookupId, setRuns]
  );
  const onRunCreated = useCallback(
    (created: TeamRunRecord) => {
      applyCreatedRunState(created, false);
    },
    [applyCreatedRunState]
  );

  const {
    refreshAgents,
    refreshTeams,
    refreshRun,
    refreshTeamRuns,
    refreshSteps,
    refreshEvents,
    refreshSnapshot,
    loadInbox,
    loadMemberEvents,
    onCreateRun: triggerCreateRun,
    onLoadRunById,
    onRefreshRuns,
    onLoadMoreRuns,
    onCancelRun,
    onResumeRun,
    onRestartRun,
  } = useTeamActions({
    token: props.token,
    selectedTeamId: effectiveSelectedTeamId,
    runContextId,
    runInput,
    runLookupId,
    runStatusFilter,
    runsLoading,
    runsHasMore,
    runsBeforeCreatedAt,
    selectedStepId,
    activeRunIdForSelectedTeam,
    activeRunForSelectedTeam,
    inboxActorId,
    inboxLimit,
    inboxAfterId,
    inboxIncludeDelivered,
    selectedMemberAgentId: selectedAgentWorkspaceEventAgentId || null,
    selectedMemberSessionId: selectedAgentWorkspaceSessionId,
    selectedMemberSnapshot: selectedAgentWorkspaceSnapshot,
    activeRunIdRef,
    eventsRef,
    memberEventsRef,
    setBusy,
    setError,
    setAgents,
    setTeams,
    setSelectedTeamId: setRouteScopedSelectedTeamId,
    setRuns,
    setTeamRunBrowserByTeam,
    setRunsLoading,
    setSteps,
    setSelectedStepId,
    setEvents,
    setEventsLoading,
    setEventsHasMore,
    setSnapshot,
    setSnapshotLoading,
    setInbox,
    setMemberEvents,
    setMemberEventsLoading,
    setMemberEventsHasMore,
    setActiveRunId,
    setRunLookupId,
    onRunCreated,
    onTeamsRefreshSettled: handleTeamsRefreshSettled,
  });

  const onGoToRuns = useCallback(() => setTab("runs"), [setTab]);

  const { onSubmitStep, onApplyStepAction } = useTeamStepActions({
    token: props.token,
    activeRunIdForSelectedTeam,
    selectedStepId,
    stepAction,
    stepKey,
    stepMemberId,
    stepDependsOn,
    stepInput,
    stepRemoteTaskId,
    stepOutput,
    stepFailText,
    stepInputReason,
    stepInputRequiredPayload,
    stepResumePayload,
    setBusy,
    setError,
    setSelectedStepId,
    refreshRun,
    refreshSteps,
    refreshEvents,
    refreshSnapshot,
  });

  const {
    onSendChatMessage,
    onSendMessage,
    onRefreshInbox,
    onAcceptMessage,
    onAcceptVisibleMessages,
  } =
    useTeamMailboxActions({
      token: props.token,
      tab,
      activeRunIdForSelectedTeam,
      chatFromActorId: chatActors.fromActorId,
      chatToActorId: chatActors.toActorId,
      chatDraft,
      msgFromActorId,
      msgToActorId,
      msgChannel,
      msgTransport,
      msgRoute,
      msgPayload,
      msgIdempotencyKey,
      inboxActorId,
      setBusy,
      setError,
      setChatDraft,
      loadInbox,
      refreshSnapshot,
      refreshEvents,
    });

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

  useTeamRunLifecycleEffects({
    token: props.token,
    selectedTeamId: effectiveSelectedTeamId,
    runStatusFilter,
    runs,
    activeRunIdForSelectedTeam,
    snapshot,
    eventsAutoRefresh,
    tab,
    chatInboxActorId: chatActors.inboxActorId,
    refreshAgents,
    refreshTeams,
    refreshTeamRuns,
    refreshRun,
    refreshEvents,
    refreshSnapshot,
    loadInbox,
    parseError: parseErrorMessage,
    setError,
    setActiveRunId,
    setRuns,
    setEvents,
    setSteps,
    setInbox,
    setSnapshot,
    setSelectedMemberId,
    setMemberEvents,
    setChatSeenByConversation,
    setChatStickToBottom,
  });

  useTeamMailboxLifecycleEffects({
    snapshot,
    selectedMemberId,
    mailboxActorIds: chatMemberIds,
    activeRunIdForSelectedTeam,
    chatInboxActorId: chatActors.inboxActorId,
    tab,
    chatStickToBottom,
    conversationKey,
    conversationLatestMessageId,
    conversationMessagesLength: conversationMessages.length,
    loadInbox,
    loadMemberEvents,
    parseError: parseErrorMessage,
    setError,
    setSelectedMemberId,
    setMemberEvents,
    setInbox,
    setInboxActorId,
    setChatStickToBottom,
    scrollConversationToBottom,
    markConversationSeen,
  });

  const onRunStatusFilterChange = useCallback(
    (nextFilter: TeamRunStatusFilter) => {
      if (!effectiveSelectedTeamId) return;
      setTeamRunBrowserByTeam((prev) => ({
        ...prev,
        [effectiveSelectedTeamId]: {
          statusFilter: nextFilter,
          beforeCreatedAt: undefined,
          hasMore: false,
        },
      }));
    },
    [effectiveSelectedTeamId]
  );

  const onApplyMessageTemplate = useCallback(() => {
    setMsgPayload(toPrettyJson(buildMailboxPayloadTemplate(msgTemplate)));
  }, [msgTemplate, setMsgPayload]);

  const {
    selectedConversation,
    selectedConversationLatestRun,
    selectedConversationId,
    workspaceTasks,
    selectedTask,
    refreshTasks,
    onRefreshTasks,
    selectedConversationDetailMissing,
  } = useTeamTaskWorkspaceData({
    token: props.token,
    effectiveSelectedTeamId,
    routeChannelId,
    routeSelectedTaskId,
    selectedChannelTaskId: selectedChannelRecord?.task_id,
    selectedConversationTaskId,
    selectedConversationDetail,
    sharedConversation,
    sharedConversationLatestRun,
    taskList,
    tasksLoading,
    selectedTaskId,
    sharedConversationRequestScopeRef,
    setError,
    setTaskList,
    setSharedConversation,
    setSharedConversationLatestRun,
    setSelectedConversationDetail,
    setTasksLoading,
    setSelectedTaskId,
    setTaskMessages,
    setConversationMailboxMessages,
    setSelectedConversationTaskId,
    setCompiledRunPreview,
    setCompilePreviewContextId,
  });
  const {
    refreshTaskMessages,
    sendTaskMessage: onSendTaskMessage,
  } = useTeamConversationActions({
    token: props.token,
    selectedTeamId: effectiveSelectedTeamId,
    selectedConversation,
    selectedConversationLatestRun,
    activeRunIdForSelectedTeam,
    refreshSnapshot,
    refreshEvents,
    setBusy,
    setError,
    setWarning,
    setSharedConversation,
    setSharedConversationLatestRun,
    setTaskMessages,
    setTaskMessagesLoading,
    setConversationMailboxMessages,
    setTaskMessageDraft,
  });
  useEffect(() => {
    if (
      !effectiveSelectedTeamId ||
      routeWorkspaceLens !== "channels" ||
      routeChannelId !== DEFAULT_TEAM_CHANNEL_ID ||
      !routeSelectedTaskId
    ) {
      return;
    }
    const owningChannelId = resolveTaskChannelId(selectedConversation);
    if (!owningChannelId) {
      return;
    }
    // Canonicalize old task-only channel routes so channel-scoped conversations
    // regain their owning lane semantics, including thread-pane behavior.
    navigateTeamRoute(
      buildTeamWorkspacePath(
        effectiveSelectedTeamId,
        "channels",
        owningChannelId,
        routeThreadRootMessageId,
        null,
        null,
        routeSelectedTaskId
      )
    );
  }, [
    effectiveSelectedTeamId,
    routeChannelId,
    routeSelectedTaskId,
    routeThreadRootMessageId,
    routeWorkspaceLens,
    selectedConversation,
  ]);

  useEffect(() => {
    if (!effectiveSelectedTeamId || !routeSelectedTaskId) {
      return;
    }
    const shouldClearRouteTask = shouldClearSelectedConversationTask({
      selectedConversationTaskId: routeSelectedTaskId,
      sharedConversationTaskId: sharedConversation?.id ?? null,
      selectedConversationDetailPresent: Boolean(selectedConversationDetail),
      selectedConversationDetailMissing,
      tasksLoading,
    });
    if (!shouldClearRouteTask) {
      return;
    }
    navigateTeamRoute(
      buildTeamWorkspacePath(effectiveSelectedTeamId, "channels", routeChannelId)
    );
  }, [
    effectiveSelectedTeamId,
    routeChannelId,
    routeSelectedTaskId,
    selectedConversationDetail,
    selectedConversationDetailMissing,
    sharedConversation?.id,
    tasksLoading,
  ]);

  useTeamConversationEffects({
    token: props.token,
    selectedTeamId: effectiveSelectedTeamId,
    selectedConversationId,
    tab,
    eventsAutoRefresh,
    refreshTaskMessages,
    setTaskMessages,
    setConversationMailboxMessages,
  });

  const onRefreshMemberConsole = useCallback(async () => {
    if (selectedAgentWorkspaceMemberId && selectedAgentWorkspaceSessionId) {
      await loadMemberEvents("replace");
      return;
    }
    if (activeRunIdForSelectedTeam) {
      await refreshEvents(activeRunIdForSelectedTeam);
    }
  }, [
    activeRunIdForSelectedTeam,
    loadMemberEvents,
    refreshEvents,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceSessionId,
  ]);

  const onLoadOlderMemberConsole = useCallback(async () => {
    if (!selectedAgentWorkspaceMemberId || !selectedAgentWorkspaceSessionId) {
      return;
    }
    await loadMemberEvents("prepend");
  }, [loadMemberEvents, selectedAgentWorkspaceMemberId, selectedAgentWorkspaceSessionId]);

  useTeamMemberAcpSessionDiscovery({
    activeRunId: activeRunIdForSelectedTeam,
    tab,
    selectedMemberId: selectedAgentWorkspaceMemberId,
    selectedSessionId: selectedAgentWorkspaceSessionId,
    snapshotStatus: selectedAgentWorkspaceSnapshot?.status ?? null,
    agentStatus: selectedAgentWorkspaceAgent?.status ?? null,
    runtimeSessionStatus: selectedAgentWorkspaceRuntimeMember?.session_status ?? null,
    runtimeAgentStatus: selectedAgentWorkspaceRuntimeMember?.agent_status ?? null,
    refreshSnapshot,
  });

  useTeamMemberAcpEffects({
    token: props.token,
    selectedAgentId: selectedAgentWorkspaceEventAgentId,
    selectedSessionId: selectedAgentWorkspaceSessionId,
    tab,
    eventsAutoRefresh,
    loadMemberEvents,
    setMemberEvents,
    setMemberEventsHasMore,
    onLiveActivity:
      tab === "agent_acp" || tab === "member_console"
        ? async () => {
            if (!activeRunIdForSelectedTeam) {
              return;
            }
            await refreshSnapshot(activeRunIdForSelectedTeam);
          }
        : undefined,
  });

  useTeamCachePersistence({
    effectiveSelectedTeamId,
    selectedConversationId,
    taskMessages,
    conversationMailboxMessages,
    setTaskMessages,
    setConversationMailboxMessages,
    selectedAgentWorkspaceEventAgentId,
    selectedAgentWorkspaceSessionId,
    memberEvents,
    setMemberEvents,
    setMemberEventsHasMore,
    activeRunIdForSelectedTeam,
    inboxActorId: chatActors.inboxActorId,
    inbox,
    setInbox,
  });

  const onRefreshOverviewSnapshot = useCallback(async () => {
    if (!activeRunIdForSelectedTeam) return;
    setError(null);
    try {
      await refreshSnapshot(activeRunIdForSelectedTeam);
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRunIdForSelectedTeam, refreshSnapshot]);

  const onRefreshActiveRunSteps = useCallback(async () => {
    if (!activeRunForSelectedTeam) {
      return;
    }
    await refreshSteps(activeRunForSelectedTeam.id);
  }, [activeRunForSelectedTeam, refreshSteps]);
  const onMailboxTemplateChange = useCallback((value: string) => {
    setMsgTemplate(value as MailboxTemplateKey);
  }, [setMsgTemplate]);

  const onRefreshEventsPanel = useCallback(async () => {
    if (!activeRunForSelectedTeam) return;
    setError(null);
    try {
      await refreshEvents(activeRunForSelectedTeam.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRunForSelectedTeam, refreshEvents]);

  const onLoadOlderEventsPanel = useCallback(async () => {
    if (!activeRunForSelectedTeam) return;
    setError(null);
    try {
      await refreshEvents(activeRunForSelectedTeam.id, "prepend");
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRunForSelectedTeam, refreshEvents]);

  const runInputValidation = useMemo(() => validateRunInputJson(runInput), [runInput]);
  const runInputHasError = runInputValidation.error !== null;
  const canCreateRun =
    busy !== "create-run" && !runInputHasError && selectedTeamHasConfiguredMembers;
  const canCompileTask =
    busy !== "compile-task" && selectedTask !== null && selectedTeamHasConfiguredMembers;
  const onCreateRun = useCallback(async () => {
    if (!selectedTeamHasConfiguredMembers) {
      setError(teamExecutionBlockedReason ?? "Add at least one agent first");
      return;
    }
    await triggerCreateRun();
  }, [selectedTeamHasConfiguredMembers, setError, teamExecutionBlockedReason, triggerCreateRun]);
  const navigateToTeamLens = useCallback(
    (
      teamId: string,
      lens: WorkspaceLens,
      channelId?: TeamChannelId | null,
      taskId?: string | null
    ) => {
      navigateTeamRoute(buildTeamLensNavigationPath(teamId, lens, channelId, taskId));
    },
    []
  );
  const navigateToSidebarTeam = useCallback(
    (teamId: string) => {
      navigateTeamRoute(buildTeamWorkspacePath(teamId, routeWorkspaceLens, routeChannelId));
    },
    [routeChannelId, routeWorkspaceLens]
  );
  const prefetchWorkspaceLens = useCallback((lens: WorkspaceLens) => {
    if (lens === "channels") {
      prefetchTeamWorkbenchTab("runs");
      prefetchTeamWorkbenchTab("mailbox");
      void loadTeamPageModals();
      return;
    }
    if (lens === "members") {
      prefetchTeamWorkbenchTab("overview");
      prefetchTeamWorkbenchTab("member_console");
      void loadTeamMemberAcpPanel();
      return;
    }
    if (lens === "tasks") {
      prefetchTeamWorkbenchTab("runs");
    }
  }, []);
  const selectedAgentWorkspaceLiveState = useMemo(
    () =>
      selectedTeamMemberLiveStates.find(
        (member) => member.member_id === selectedAgentWorkspaceMemberId
      ) ?? null,
    [selectedAgentWorkspaceMemberId, selectedTeamMemberLiveStates]
  );
  const {
    activeWorkspaceLens,
    isAgentWorkspace,
    selectedAgentLabel,
    selectedAgentSpecDraft,
    selectedAgentStatusView,
    activeConversationTitle,
    selectedConversationIsShared,
    workspaceTitle,
    workspaceDescription,
    showWorkspaceRuntimeBadge,
    showDedicatedWorkspaceHeading,
    workspaceNoticeText,
    workspaceNoticeDotClassName,
    workspaceDetailItems,
    mailboxDisplayNameByActorId,
    onOpenTaskRun,
    onOpenMailboxForMember,
    onSelectConversationSubject,
    onSelectKanbanSubject,
    onSelectAgentWorkspace,
    onSelectSidebarTeam,
    showRunContextLoading,
    showNoActiveRunNotice,
  } = useTeamWorkspaceViewModel({
    selectedTeam,
    routeWorkspaceLens,
    tab,
    focusedAgentMemberId,
    selectedMemberId,
    selectedTeamId,
    selectedTeamMemberLiveStates,
    selectedTeamMemberSummary,
    selectedTeamRuntimeStatus,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceLiveState,
    activeRunForSelectedTeam,
    activeRunIdForSelectedTeam,
    selectedConversation,
    selectedChannelId: selectedChannelItem.id,
    selectedChannelLabel: selectedChannelItem.label,
    selectedChannelDescription: selectedChannelItem.description,
    runsLoading,
    isCompactWorkbench,
    teamPromptDefaults,
    teamMemberAgentsById,
    agents,
    setTab,
    setFocusedAgentMemberId,
    setSelectedConversationTaskId,
    setSelectedMemberId,
    setTeamsSidebarCollapsed,
    setActiveRunId,
    setRunLookupId,
    navigateToTeamLens,
    navigateToTeamDetail,
    navigateToTeamMemberWorkspace,
    navigateToSidebarTeam,
    prefetchWorkspaceLens,
  });
  const onSelectSidebarChannel = useCallback(
    (channelId: TeamChannelId) => {
      setFocusedAgentMemberId("");
      setSelectedConversationTaskId("");
      setTab("conversation");
      if (effectiveSelectedTeamId) {
        navigateTeamRoute(buildTeamWorkspacePath(effectiveSelectedTeamId, "channels", channelId));
      }
      if (isCompactWorkbench) {
        setTeamsSidebarCollapsed(true);
      }
    },
    [
      effectiveSelectedTeamId,
      isCompactWorkbench,
      setFocusedAgentMemberId,
      setSelectedConversationTaskId,
      setTab,
      setTeamsSidebarCollapsed,
    ]
  );
  const onCreateSidebarChannel = useCallback(
    async (payload: { channelId: string; description: string }) => {
      if (!effectiveSelectedTeamId) {
        const error = new Error("Select a team first");
        setError(error.message);
        throw error;
      }
      setError(null);
      setBusy("create-team-channel");
      try {
        const created = await api.createTeamChannel(props.token, effectiveSelectedTeamId, {
          channel_id: payload.channelId,
          description: payload.description || null,
        });
        await refreshTeamChannels(effectiveSelectedTeamId);
        setFocusedAgentMemberId("");
        setSelectedConversationTaskId(created.task_id);
        setTab("conversation");
        navigateTeamRoute(
          buildTeamWorkspacePath(effectiveSelectedTeamId, "channels", created.channel_id)
        );
        if (isCompactWorkbench) {
          setTeamsSidebarCollapsed(true);
        }
      } catch (err) {
        setError(parseErrorMessage(err));
        throw err;
      } finally {
        setBusy(null);
      }
    },
    [
      effectiveSelectedTeamId,
      isCompactWorkbench,
      props.token,
      refreshTeamChannels,
      setFocusedAgentMemberId,
      setSelectedConversationTaskId,
      setTab,
      setTeamsSidebarCollapsed,
    ]
  );
  const onDeleteSidebarChannel = useCallback(
    async (channelId: TeamChannelId) => {
      if (!effectiveSelectedTeamId || channelId === DEFAULT_TEAM_CHANNEL_ID) {
        return;
      }
      setError(null);
      setBusy("delete-team-channel");
      setDeletingChannelId(channelId);
      try {
        await api.deleteTeamChannel(props.token, effectiveSelectedTeamId, channelId);
        await refreshTeamChannels(effectiveSelectedTeamId);
        if (routeChannelId === channelId && routeWorkspaceLens === "channels") {
          setSelectedConversationTaskId("");
          navigateTeamRoute(buildTeamWorkspacePath(effectiveSelectedTeamId, "channels"));
        }
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
        setDeletingChannelId(null);
        setBusy(null);
      }
    },
    [
      effectiveSelectedTeamId,
      props.token,
      refreshTeamChannels,
      routeChannelId,
      routeWorkspaceLens,
      setSelectedConversationTaskId,
    ]
  );
  useEffect(() => {
    if (!selectedTeam) {
      return;
    }

    let active = true;
    const idleWindow = window as Window & {
      requestIdleCallback?: (callback: () => void, options?: { timeout: number }) => number;
      cancelIdleCallback?: (handle: number) => void;
    };
    const prefetchCommonSurfaces = () => {
      if (!active) {
        return;
      }
      prefetchTeamWorkbenchTab("runs");
      prefetchTeamWorkbenchTab("overview");
      void loadTeamMemberAcpPanel();
      void loadTeamPageModals();
      if (!selectedTeamHasConfiguredMembers) {
        prefetchTeamSetupSurface();
      }
    };

    let idleHandle: number | null = null;
    let timeoutHandle: number | null = null;
    if (typeof idleWindow.requestIdleCallback === "function") {
      idleHandle = idleWindow.requestIdleCallback(prefetchCommonSurfaces, { timeout: 1200 });
    } else {
      timeoutHandle = window.setTimeout(prefetchCommonSurfaces, 400);
    }

    return () => {
      active = false;
      if (idleHandle != null && typeof idleWindow.cancelIdleCallback === "function") {
        idleWindow.cancelIdleCallback(idleHandle);
      }
      if (timeoutHandle != null) {
        window.clearTimeout(timeoutHandle);
      }
    };
  }, [selectedTeam, selectedTeamHasConfiguredMembers]);
  const workspaceAdvancedTabItems = (isAgentWorkspace
    ? TEAM_AGENT_ADVANCED_TAB_ITEMS
    : TEAM_UTILITY_ADVANCED_TAB_ITEMS
  ).filter((item) => props.developerMode || item.value !== "debug");
  const isAdvancedWorkspace = workspaceAdvancedTabItems.some((item) => item.value === tab);
  const showRunActionsInAdvanced = Boolean(activeRunForSelectedTeam && tab !== "runs");
  const workspaceEyebrow = null;
  const {
    openCreateTeamModal,
    closeCreateTeamModal,
    openTeamMemberForgeModal,
    openCopyExistingAgentModal,
    closeCopyExistingAgentModal,
    closeTeamMemberForgeModal,
    openTeamMemberEditModal,
    closeTeamMemberEditModal,
    refreshTeamRuntime,
    onCreateForgeAgent,
    onCopyExistingTeamAgent,
    onSaveTeamMemberProfile,
    onCreateTeam,
    onDeleteTeam,
    onStartSelectedTeamAgent,
    onStopSelectedTeamAgent,
    onDeleteSelectedTeamAgent,
    onForceNewTeamMemberSession,
    onStartTeamRuntime,
    onStopTeamRuntime,
  } = useTeamManagementActions({
    token: props.token,
    busy,
    agents,
    teams,
    runs,
    selectedTeam,
    selectedTeamId,
    selectedTeamHasCoordinator,
    selectedTeamHasConfiguredMembers,
    teamExecutionBlockedReason,
    selectedTeamWorkerCount,
    selectedTeamMemberStatuses,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceAgent,
    selectedAgentLabel,
    newTeamName,
    newTeamDescription,
    teamMemberDraft,
    teamMemberEditDraft,
    teamPromptDefaults,
    forgeDefaultWorktreeRoot,
    forgeAgentName,
    forgeAgentWorkdir,
    forgeAgentPresetId,
    forgeAgentCodexAcpDefaultMode,
    forgeAgentWorktreeMode,
    forgeAgentWorktreeRepo,
    forgeAgentWorktreeRef,
    forgeAgentCodeMode,
    forgeAgentBusy,
    patchTeamCreate,
    resetTeamDraft,
    refreshTeams,
    refreshAgents,
    navigateToTeamDetail,
    navigateToTeamSelector,
    setError,
    setWarning,
    setBusy,
    setAgents,
    setTeams,
    setSelectedTeamId,
    setRuns,
    setTeamRunBrowserByTeam,
    setActiveRunId,
    setRunLookupId,
    setTeamSelectorFilter,
    setTeamMemberDraft,
    setTeamMemberEditDraft,
    setShowTeamMemberEditModal,
    setTeamRuntimeByTeamId,
    setShowCreateTeamModal,
    setShowForgeAgentForm,
    setShowCopyExistingAgentModal,
    setForgeAgentName,
    setForgeAgentWorkdir,
    setForgeAgentPresetId,
    setForgeAgentCodexAcpDefaultMode,
    setForgeAgentWorktreeMode,
    setForgeAgentWorktreeRepo,
    setForgeAgentWorktreeRef,
    setForgeAgentCodeMode,
    setForgeAgentWorktreeError,
    setForgeAgentBusy,
    setTeamMemberAgentsById,
    setMemberDiscoveryCardsById,
    setMemberDiscoveryCardLoadingById,
  });
  React.useEffect(() => {
    setWorkspaceDetailsOpen(false);
  }, [props.developerMode, selectedTeamId, tab]);
  React.useEffect(() => {
    if (!props.developerMode && tab === "debug") {
      setTab("conversation");
      setWorkspaceDetailsOpen(false);
    }
  }, [props.developerMode, setTab, tab]);
  const onRefreshActiveRun = useCallback(() => {
    if (!activeRunIdForSelectedTeam) return;
    void refreshRun(activeRunIdForSelectedTeam).catch((err) => setError(parseErrorMessage(err)));
  }, [activeRunIdForSelectedTeam, refreshRun, setError]);
  const onSendAgentAcpInput = useCallback(
    async (text: string, sessionId: string) => {
      const agentId = selectedAgentWorkspaceEventAgentId.trim();
      const normalizedText = text.trim();
      if (!props.token || !agentId || !normalizedText || !sessionId) {
        return;
      }
      setError(null);
      const messageId =
        typeof crypto !== "undefined" && "randomUUID" in crypto
          ? crypto.randomUUID()
          : `team-agent-acp-${Date.now()}`;
      const sendForSession = (nextSessionId: string) =>
        api.sendInput(props.token, agentId, normalizedText, messageId, nextSessionId);
      try {
        await sendForSession(sessionId);
        await loadMemberEvents("replace");
      } catch (err) {
        const msg = parseErrorMessage(err);
        const mismatch = parseTeamAgentInputSessionMismatch(msg);
        if (mismatch) {
          try {
            await sendForSession(mismatch.running);
            setSelectedAgentWorkspaceStickySession({
              memberId: selectedAgentWorkspaceMemberId.trim(),
              sessionId: mismatch.running,
            });
            setSelectedAgentWorkspaceSessionOverride({
              memberId: selectedAgentWorkspaceMemberId.trim(),
              sessionId: mismatch.running,
            });
            if (selectedTeamId) {
              setTeamRuntimeByTeamId((prev) => {
                const runtime = prev[selectedTeamId];
                if (!runtime) return prev;
                let changed = false;
                const members = runtime.members.map((member) => {
                  if (member.member_id !== selectedAgentWorkspaceMemberId) {
                    return member;
                  }
                  if (member.session_id === mismatch.running) {
                    return member;
                  }
                  changed = true;
                  return {
                    ...member,
                    session_id: mismatch.running,
                  };
                });
                if (!changed) return prev;
                return {
                  ...prev,
                  [selectedTeamId]: {
                    ...runtime,
                    members,
                  },
                };
              });
              void refreshTeamRuntime(selectedTeamId).catch(() => undefined);
            }
            await loadMemberEvents("replace", mismatch.running);
            return;
          } catch (retryErr) {
            setError(parseErrorMessage(retryErr));
            return;
          }
        }
        await loadMemberEvents("replace").catch(() => undefined);
        setError(msg);
        if (msg.includes(AGENT_NOT_RUNNING_ERROR)) {
          if (selectedTeamId) {
            void refreshTeamRuntime(selectedTeamId).catch(() => undefined);
          }
          void refreshAgents().catch(() => undefined);
        }
      }
    },
    [
      loadMemberEvents,
      props.token,
      refreshAgents,
      refreshTeamRuntime,
      selectedAgentWorkspaceEventAgentId,
      selectedAgentWorkspaceMemberId,
      selectedTeamId,
      setError,
    ]
  );
  const onCancelTeamMemberAcp = useCallback(async () => {
    if (!props.token || !selectedAgentWorkspaceAgent) {
      return;
    }
    setError(null);
    try {
      await api.cancelAcp(props.token, selectedAgentWorkspaceAgent.id);
      if (selectedTeamId) {
        void refreshTeamRuntime(selectedTeamId).catch(() => undefined);
      }
      void refreshAgents().catch(() => undefined);
      await loadMemberEvents("replace");
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [
    loadMemberEvents,
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentWorkspaceAgent,
    selectedTeamId,
  ]);
  const onSetTeamMemberAcpMode = useCallback(async (modeId: string) => {
    if (!props.token || !selectedAgentWorkspaceAgent) {
      return;
    }
    const trimmedModeId = modeId.trim();
    if (!trimmedModeId) {
      setError("mode id is required");
      return;
    }
    setError(null);
    try {
      await api.setAcpMode(props.token, selectedAgentWorkspaceAgent.id, trimmedModeId);
      await loadMemberEvents("replace");
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [loadMemberEvents, props.token, selectedAgentWorkspaceAgent]);
  const onSetTeamMemberAcpModel = useCallback(async (modelId: string) => {
    if (!props.token || !selectedAgentWorkspaceAgent) {
      return;
    }
    const trimmedModelId = modelId.trim();
    if (!trimmedModelId) {
      setError("model id is required");
      return;
    }
    setError(null);
    try {
      await api.setAcpModel(props.token, selectedAgentWorkspaceAgent.id, trimmedModelId);
      await loadMemberEvents("replace");
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [loadMemberEvents, props.token, selectedAgentWorkspaceAgent]);
  const onSetTeamMemberAcpConfig = useCallback(async (
    configId: string,
    value: string
  ) => {
    if (!props.token || !selectedAgentWorkspaceAgent) {
      return;
    }
    const trimmedConfigId = configId.trim();
    const trimmedValue = value.trim();
    if (!trimmedConfigId || !trimmedValue) {
      setError("config id and value are required");
      return;
    }
    setError(null);
    try {
      await api.setAcpConfig(
        props.token,
        selectedAgentWorkspaceAgent.id,
        trimmedConfigId,
        trimmedValue
      );
      await loadMemberEvents("replace");
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [loadMemberEvents, props.token, selectedAgentWorkspaceAgent]);
  const selectedAgentControlState = useMemo(
    () =>
      resolveTeamMemberAgentControlState(
        selectedAgentWorkspaceAgent,
        selectedAgentStatusView.lifecycle,
        busy
      ),
    [busy, selectedAgentStatusView.lifecycle, selectedAgentWorkspaceAgent]
  );
  useEffect(() => {
    if (!selectedTeamId) {
      return;
    }
    let active = true;
    void refreshTeamRuntime(selectedTeamId, { apply: false })
      .then((runtime) => {
        if (!active) {
          return;
        }
        setTeamRuntimeByTeamId((prev) => ({ ...prev, [selectedTeamId]: runtime }));
      })
      .catch((err) => {
        if (!active) {
          return;
        }
        setError(parseErrorMessage(err));
      });
    return () => {
      active = false;
    };
  }, [refreshTeamRuntime, selectedTeamId, setError]);
  useTeamRuntimeEffects({
    token: props.token,
    selectedTeamId,
    enabled: shouldWatchSelectedTeamRuntime,
    refreshTeamRuntime,
    onRefreshError: (err) => {
      setError(parseErrorMessage(err));
    },
  });
  useTeamTaskEffects({
    selectedTeamId,
    enabled: tab === "tasks",
    refreshTasks,
    onRefreshError: (err) => {
      setError(parseErrorMessage(err));
    },
  });
  const onCompileTaskRunPreview = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    if (!selectedTeamHasConfiguredMembers) {
      setError(teamExecutionBlockedReason ?? "Add at least one agent first");
      return;
    }
    const taskId = selectedTask?.id ?? "";
    if (!taskId) {
      setError("Select a task first");
      return;
    }
    setBusy("compile-task");
    setError(null);
    try {
      const preview = await api.compileTeamTaskRunPreview(
        props.token,
        selectedTeamId,
        taskId,
        {
          context_id: compilePreviewContextId.trim() || undefined,
        }
      );
      setCompiledRunPreview(preview);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    compilePreviewContextId,
    props.token,
    selectedTeamHasConfiguredMembers,
    selectedTask?.id,
    selectedTeamId,
    setBusy,
    setError,
    teamExecutionBlockedReason,
  ]);
  const onUseCompiledRunPayload = useCallback(() => {
    if (!compiledRunPreview) {
      return;
    }
    setRunContextId(compiledRunPreview.run_payload.context_id);
    setRunInput(toPrettyJson(compiledRunPreview.run_payload.input));
  }, [compiledRunPreview, setRunContextId, setRunInput]);
  const onCreateRunFromCompiledPreview = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    if (!selectedTeamHasConfiguredMembers) {
      setError(teamExecutionBlockedReason ?? "Add at least one agent first");
      return;
    }
    if (!compiledRunPreview) {
      setError("Compile preview first");
      return;
    }
    setBusy("create-run");
    setError(null);
    try {
      const created = await api.createTeamRun(props.token, selectedTeamId, {
        context_id: compiledRunPreview.run_payload.context_id,
        input: compiledRunPreview.run_payload.input,
      });
      applyCreatedRunState(created, true);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    applyCreatedRunState,
    compiledRunPreview,
    props.token,
    selectedTeamHasConfiguredMembers,
    selectedTeamId,
    setBusy,
    setError,
    teamExecutionBlockedReason,
  ]);
  const onSendThreadReply = useCallback(async (payload: { text: string; mentionActorIds: string[] }) => {
    if (!effectiveSelectedTeamId || !routeThreadRootMessageId) {
      setError("Open a thread first");
      return;
    }
    const text = payload.text.trim();
    if (!text) {
      setError("Thread reply is required");
      return;
    }
    setBusy("send-thread-reply");
    setError(null);
    try {
      await api.replyTeamThread(
        props.token,
        effectiveSelectedTeamId,
        selectedChannelItem.id,
        routeThreadRootMessageId,
        { text, mention_actor_ids: payload.mentionActorIds }
      );
      setThreadReplyDraft("");
      if (typeof EventSource === "undefined") {
        await refreshTaskMessages(selectedConversation?.id ?? undefined);
      }
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    effectiveSelectedTeamId,
    props.token,
    refreshTaskMessages,
    routeThreadRootMessageId,
    selectedChannelItem.id,
    selectedConversation?.id,
    setBusy,
    setError,
  ]);
  useEffect(() => {
    setThreadReplyDraft("");
  }, [routeThreadRootMessageId]);
  const selectedConversationMatchesChannelLane = Boolean(
    channelWorkspaceActive &&
      selectedConversation &&
      (
        (routeChannelId === DEFAULT_TEAM_CHANNEL_ID && selectedConversationIsShared) ||
        selectedChannelRecord?.task_id === selectedConversation.id ||
        isChannelScopedConversationTask(selectedConversation, routeChannelId)
      )
  );
  const activeChannelConversationTaskId =
    resolveChannelRouteTaskId({
      routeSelectedTaskId,
      selectedConversationTaskId: selectedConversation?.id,
      selectedConversationIsShared,
      selectedConversationMatchesChannelLane,
      selectedChannelTaskId: selectedChannelRecord?.task_id,
    });
  const [channelFocusMessageId, setChannelFocusMessageId] = React.useState<number | null>(null);

  const warningNotice = resolveTeamPageNotice(warning);
  const showSidebarPane = !isSelectorRoute && !teamsSidebarCollapsed;
  const showWorkbenchPane = !isCompactWorkbench || teamsSidebarCollapsed;
  const showTeamBootstrapLoading =
    !isSelectorRoute &&
    Boolean(effectiveSelectedTeamId) &&
    !selectedTeam &&
    !teamCatalogSettled;
  const showTeamUnavailable =
    !isSelectorRoute &&
    Boolean(effectiveSelectedTeamId) &&
    !selectedTeam &&
    teamCatalogSettled;
  const detailLayoutClassName = isCompactWorkbench
    ? "teams-layout flex min-h-0 flex-1 flex-col"
    : teamsSidebarCollapsed
      ? "teams-layout flex min-h-0 flex-1 flex-col"
      : "teams-layout grid min-h-0 flex-1 grid-cols-[var(--teams-sidebar-width,260px)_1fr]";

  const modalChrome = useMemo(
    () => ({
      panelClassName: teamWorkbenchPanelClassName,
      accentButtonClassName: teamWorkbenchAccentButtonClassName,
      mutedButtonClassName: teamWorkbenchMutedButtonClassName,
      badgeClassName: teamWorkbenchBadgeClassName,
      modalHeaderClassName:
        "flex items-start justify-between gap-3 border-b border-notion-border px-5 py-4",
      setupChecklistClassName: teamWorkbenchSetupChecklistClassName,
      infoStripGridClassName: teamWorkbenchInfoStripGridClassName,
      infoStripItemClassName: teamWorkbenchInfoStripItemClassName,
      infoStripLabelClassName: teamWorkbenchInfoStripLabelClassName,
      infoStripValueClassName: teamWorkbenchInfoStripValueClassName,
    }),
    []
  );

  const forgeModalProps = useMemo(
    () =>
      buildCreateAgentModalProps({
        teamStyled: true,
        agentName: forgeAgentName,
        setAgentName: setForgeAgentName,
        agentWorkdir: forgeAgentWorkdir,
        setAgentWorkdir: setForgeAgentWorkdir,
        agentPresetId: forgeAgentPresetId,
        setAgentPresetId: setForgeAgentPresetId,
        codexAcpDefaultMode: forgeAgentCodexAcpDefaultMode,
        setCodexAcpDefaultMode: setForgeAgentCodexAcpDefaultMode,
        worktreeMode: forgeAgentWorktreeMode,
        setWorktreeMode: handleForgeWorktreeModeChange,
        worktreeRepo: forgeAgentWorktreeRepo,
        setWorktreeRepo: setForgeAgentWorktreeRepo,
        worktreeRef: forgeAgentWorktreeRef,
        setWorktreeRef: setForgeAgentWorktreeRef,
        codeMode: forgeAgentCodeMode,
        setCodeMode: setForgeAgentCodeMode,
        worktreeError: forgeAgentWorktreeError,
        agentPresetLabel: "Runtime",
        agentPresetSummaryLabel: "Model",
        showWorktreeAdvancedOptions: teamMemberDraft?.role !== "coordinator",
        createBusy: forgeAgentBusy,
        workdirPlaceholder: forgeDefaultWorktreeRoot,
        withinPortal: true,
        onClose: closeTeamMemberForgeModal,
        onCreateAgent: onCreateForgeAgent,
      }),
    [
      forgeAgentBusy,
      forgeAgentCodeMode,
      forgeAgentName,
      forgeAgentPresetId,
      forgeAgentCodexAcpDefaultMode,
      forgeAgentWorkdir,
      forgeAgentWorktreeError,
      forgeAgentWorktreeMode,
      forgeAgentWorktreeRef,
      forgeAgentWorktreeRepo,
      forgeDefaultWorktreeRoot,
      handleForgeWorktreeModeChange,
      closeTeamMemberForgeModal,
      onCreateForgeAgent,
      setForgeAgentCodeMode,
      setForgeAgentName,
      setForgeAgentPresetId,
      setForgeAgentCodexAcpDefaultMode,
      setForgeAgentWorkdir,
      setForgeAgentWorktreeRef,
      setForgeAgentWorktreeRepo,
      teamMemberDraft?.role,
    ]
  );

  const teamPageModalsProps = buildTeamPageModalsProps({
    showCreateTeamModal,
    showForgeAgentForm,
    showCopyExistingAgentModal,
    showTeamMemberEditModal,
    busy,
    newTeamName,
    newTeamDescription,
    onTeamNameChange: setNewTeamName,
    onTeamDescriptionChange: setNewTeamDescription,
    onCreateTeam,
    closeCreateTeamModal,
    teamMemberDraft,
    teamMemberRoleProfile,
    selectedTeamHasCoordinator,
    copyExistingCandidates: copyExistingTeamAgentCandidates,
    patchTeamMemberDraft,
    forgeModalProps,
    closeTeamMemberForgeModal,
    closeCopyExistingAgentModal,
    onCopyExistingAgent: onCopyExistingTeamAgent,
    selectedAgentLabel,
    teamMemberEditDraft,
    patchTeamMemberEditDraft,
    closeTeamMemberEditModal,
    onSaveTeamMemberProfile,
    createChrome: modalChrome,
    forgeChrome: modalChrome,
    editChrome: modalChrome,
  });
  const hasOpenTeamModal =
    showCreateTeamModal ||
    showForgeAgentForm ||
    showCopyExistingAgentModal ||
    showTeamMemberEditModal;
  const normalizedTeamPageError = useMemo(() => {
    if (!error) {
      return null;
    }
    const message = sanitizeErrorBannerMessage(error, getNavigatorOnline());
    return shouldHideErrorBannerMessage(message) ? null : message;
  }, [error]);

  const teamPanelToggleLabel = isCompactWorkbench
    ? teamsSidebarCollapsed
      ? "Show teams panel"
      : "Show workbench"
    : teamsSidebarCollapsed
      ? "Show teams panel"
      : "Hide teams panel";
  const toggleWorkspaceDetails = useCallback(() => {
    setWorkspaceDetailsOpen((current) => !current);
  }, [setWorkspaceDetailsOpen]);
  const teamWorkbenchContext = useShallowStableObject<TeamWorkbenchRuntimeContext>({
    showTeamBootstrapLoading,
    showTeamUnavailable,
    onBackToSelector: navigateToTeamSelector,
    selectedTeam,
    isAgentWorkspace,
    teamSectionCardClassName: TEAM_PANEL_CARD_CLASS,
    teamSectionTitleClassName: TEAM_PANEL_TITLE_CLASS,
    teamSectionBodyTextClassName: TEAM_SECTION_BODY_TEXT_CLASS,
    panelSecondaryButtonClassName: TEAM_PANEL_SECONDARY_BUTTON_CLASS,
    teamWorkbenchWorkspaceShellClassName,
    selectedTeamHasConfiguredMembers,
    selectedTeamDescription: selectedTeam?.description,
    teamMemberForgeLabel,
    teamMemberCopyExistingLabel,
    onOpenTeamMemberForge: openTeamMemberForgeModal,
    onOpenTeamMemberCopyExisting: openCopyExistingAgentModal,
    tab,
    selectedTeamRuntimeStatus,
    selectedTeamRuntimeControlTone,
    showRunContextLoading,
    showNoActiveRunNotice,
    activeWorkspaceLens,
    developerMode: props.developerMode,
    onGoToRuns,
    workspaceEyebrow,
    showDedicatedWorkspaceHeading,
    workspaceTitle,
    workspaceDescription,
    selectedAgentLabel,
    selectedAgentWorkspaceMemberId,
    selectedAgentStatusView,
    selectedAgentSpecDraft,
    selectedAgentControlState,
    showWorkspaceRuntimeBadge,
    workspaceAdvancedTabItems,
    isAdvancedWorkspace,
    showRunActionsInAdvanced,
    canResumeActiveRun,
    canRestartActiveRun,
    workspaceDetailsOpen,
    workspaceDetailItems,
    workspaceNoticeText,
    workspaceNoticeDotClassName,
    teamWorkbenchMutedButtonClassName,
    teamWorkbenchHeaderActionButtonClassName,
    workspaceToolbarClassName,
    workspaceToolbarButtonActiveClassName,
    workspaceToolbarButtonIdleClassName,
    workspaceNoticeClassName,
    workspaceNoticeTextClassName,
    teamRunMetaItemClassName,
    onTabChange: setTab,
    onToggleWorkspaceDetails: toggleWorkspaceDetails,
    onRefreshActiveRun,
    onCancelRun,
    onResumeRun,
    onRestartRun,
    onOpenTeamMemberEditModal: openTeamMemberEditModal,
    onStartSelectedTeamAgent,
    onStopSelectedTeamAgent,
    onDeleteSelectedTeamAgent,
    onDeleteTeam,
    runStatusFilter,
    TEAM_RUN_STATUS_FILTER_OPTIONS,
    onRunStatusFilterChange,
    onRefreshRuns,
    runsLoading,
    visibleRuns,
    activeRunIdForSelectedTeam,
    setActiveRunId,
    isActiveRunHiddenByFilter,
    activeRunForSelectedTeam,
    totalLoadedRunsForTeam,
    runsHasMore,
    effectiveSelectedTeamId,
    onLoadMoreRuns,
    TEAM_EVENT_PREVIEW_LIMIT,
    selectedMemberDiscoveryCard,
    selectedMemberDiscoveryCardLoading,
    onOpenMailboxForMember,
    selectedMemberId,
    setSelectedMemberId,
    eventsAutoRefresh,
    setEventsAutoRefresh,
    onRefreshEventsPanel,
    onLoadOlderEventsPanel,
    eventsHasMore,
    mailboxHasActiveRun: Boolean(activeRunForSelectedTeam),
    mailboxEmptyTitle: isAgentWorkspace ? selectedAgentLabel : "Execution Mailbox",
    mailboxEmptyBody: isAgentWorkspace
      ? "This agent is selected, but there is no active execution run context for its direct thread yet. Use Execution Runs to inspect execution history or wait for the next task."
      : "Execution mailbox is run-scoped. Start or select a run to inspect delivery and direct member conversations.",
    snapshot,
    mailboxDisplayNameByActorId,
    busy,
    selectedAgentWorkspaceSessionId,
    memberEvents,
    memberEventsLoading,
    memberEventsHasMore,
    onLoadOlderMemberConsole,
    onRefreshMemberConsole,
    teamDebugTag,
    setTeamDebugTag,
    runContextId,
    setRunContextId,
    runInput,
    setRunInput,
    runLookupId,
    setRunLookupId,
    canCreateRun,
    runInputHasError,
    runInputValidation,
    teamExecutionBlockedReason,
    onCreateRun,
    onLoadRunById,
    steps,
    onRefreshActiveRunSteps,
    stepKey,
    setStepKey,
    stepMemberId,
    onStepMemberIdChange: setStepMemberId,
    stepDependsOn,
    onStepDependsOnChange: setStepDependsOn,
    stepInput,
    onStepInputChange: setStepInput,
    onSubmitStep,
    selectedStepId,
    setSelectedStepId,
    stepAction,
    setStepAction,
    stepRemoteTaskId,
    onStepRemoteTaskIdChange: setStepRemoteTaskId,
    stepOutput,
    onStepOutputChange: setStepOutput,
    stepFailText,
    onStepFailTextChange: setStepFailText,
    stepInputReason,
    onStepInputReasonChange: setStepInputReason,
    stepInputRequiredPayload,
    onStepInputRequiredPayloadChange: setStepInputRequiredPayload,
    stepResumePayload,
    onStepResumePayloadChange: setStepResumePayload,
    onApplyStepAction,
    unreadByMemberId,
    chatActors,
    chatStickToBottom,
    chatMessagesRef,
    onConversationScroll,
    onJumpConversationToBottom,
    conversationMessages,
    onAcceptMessage,
    onAcceptVisibleMessages,
    onSendChatMessage,
    MAILBOX_TEMPLATE_OPTIONS,
    onMailboxTemplateChange,
    onApplyMessageTemplate,
    onSendMessage,
    onRefreshInbox,
    selectedAgentWorkspaceSnapshot,
    selectedMemberSnapshot,
    selectedAgentWorkspaceRuntimeMember,
    selectedAgentWorkspaceAgent,
    oldestMemberEventId,
    onSendAgentAcpInput,
    onCancelTeamMemberAcp,
    onSetTeamMemberAcpMode,
    onSetTeamMemberAcpModel,
    onSetTeamMemberAcpConfig,
    onForceNewTeamMemberSession,
    eventsLoading,
    oldestEventId,
    displayedRunEvents,
    previewMode,
    snapshotLoading,
    onRefreshOverviewSnapshot,
    memberTargetNodeById,
    msgFromActorId,
    onMsgFromActorIdChange: setMsgFromActorId,
    msgToActorId,
    onMsgToActorIdChange: setMsgToActorId,
    msgChannel,
    onMsgChannelChange: setMsgChannel,
    msgTransport,
    onMsgTransportChange: setMsgTransport,
    msgRoute,
    onMsgRouteChange: setMsgRoute,
    msgTemplate,
    msgPayload,
    onMsgPayloadChange: setMsgPayload,
    msgIdempotencyKey,
    onMsgIdempotencyKeyChange: setMsgIdempotencyKey,
    inboxActorId,
    onInboxActorIdChange: setInboxActorId,
    inboxLimit,
    onInboxLimitChange: setInboxLimit,
    inboxAfterId,
    onInboxAfterIdChange: setInboxAfterId,
    inboxIncludeDelivered,
    onInboxIncludeDeliveredChange: setInboxIncludeDelivered,
    chatDraft,
    onChatDraftChange: setChatDraft,
  });
  const teamWorkspaceContext = useShallowStableObject<TeamWorkspaceContextValue>({
    workbench: teamWorkbenchContext,
    selectedConversation,
    developerMode: props.developerMode,
    token: props.token,
    tasksLoading,
    onRefreshTasks,
    taskMessageDraft,
    setTaskMessageDraft,
    onSendTaskMessage,
    taskMessages,
    conversationMailboxMessages,
    snapshot,
    mailboxDisplayNameByActorId,
    selectedTeamMemberLiveStates,
    taskConversationMemberIds,
    activeConversationTitle,
    selectedConversationMatchesChannelLane,
    taskMessagesLoading,
    busy,
    routeThreadRootMessageId,
    channelFocusMessageId,
    setChannelFocusMessageId,
    effectiveSelectedTeamId,
    routeWorkspaceLens,
    routeChannelId,
    activeChannelConversationTaskId,
    navigateTeamRoute,
    isCompactWorkbench,
    selectedChannelItem,
    workspaceTasks,
    selectedTaskId,
    setSelectedTaskId,
    onSelectConversationSubject,
    runs,
    onOpenTaskRun,
    compilePreviewContextId,
    setCompilePreviewContextId,
    onCompileTaskRunPreview,
    canCompileTask,
    compiledRunPreview,
    onUseCompiledRunPayload,
    onCreateRunFromCompiledPreview,
    onSendThreadReply,
    threadReplyDraft,
    setThreadReplyDraft,
  });

  return (
    <WorkspaceShell
      title={isSelectorRoute ? "Teams" : "Workspace"}
      subtitle={null}
      activeSurface="teams"
      username={props.auth.username}
      isRoot={props.auth.role === "root"}
      agentsCollapsed={teamsSidebarCollapsed}
      onToggleAgents={() => setTeamsSidebarCollapsed((previous) => !previous)}
      sidebarToggleLabel={teamPanelToggleLabel}
      normalizedError={normalizedTeamPageError}
      onClearError={() => setError(null)}
      onLogout={props.onLogout}
      onNavigate={navigateTeamRoute}
      warningNotice={
        warningNotice?.kind === "runtime" ? (
          <div className={teamRuntimeNoticeClassName} role="status">
            <div className="min-w-0 flex-1">
              <div className={teamRuntimeNoticeTitleClassName}>{warningNotice.title}</div>
              <div className={teamRuntimeNoticeBodyClassName}>{warningNotice.message}</div>
            </div>
            <IconButton
              tone="subtle"
              size="md"
              className="h-8 w-8 shrink-0 rounded-full border border-emerald-200 bg-white/80 text-emerald-700 hover:bg-white"
              aria-label="Dismiss runtime notice"
              onClick={() => setWarning(null)}
            >
              <i className="bi bi-x-lg" aria-hidden="true" />
            </IconButton>
          </div>
        ) : warningNotice?.kind === "warning" ? (
          <Alert
            color="yellow"
            variant="light"
            radius="xl"
            role="status"
            title={warningNotice.title}
            icon={<i className="bi bi-exclamation-triangle" aria-hidden="true" />}
            withCloseButton
            onClose={() => setWarning(null)}
          >
            <span className="text-sm text-amber-900">{warningNotice.message}</span>
          </Alert>
        ) : null
      }
      modals={
        hasOpenTeamModal && (
          <Suspense fallback={null}>
            <LazyTeamPageModals {...teamPageModalsProps} />
          </Suspense>
        )
      }
    >
      {isSelectorRoute ? (
        <TeamSelectorPanel
          busy={busy}
          filter={teamSelectorFilter}
          loading={!teamCatalogSettled && teams.length === 0}
          hasTeams={teams.length > 0}
          items={selectorTeamItems}
          onFilterChange={setTeamSelectorFilter}
          onRefreshTeams={() => {
            void refreshTeams();
          }}
          onCreateTeam={openCreateTeamModal}
          onSelectTeam={navigateToTeamDetail}
        />
      ) : (
        <div className={detailLayoutClassName}>
          {showSidebarPane ? (
            <TeamSidebarContainer
              isRoot={props.auth.role === "root"}
              developerMode={props.developerMode}
              busy={busy}
              refreshTeams={refreshTeams}
              openCreateTeamModal={openCreateTeamModal}
              newTeamName={newTeamName}
              coordinatorMemberId={
                selectedTeamMembers.find((member) => member.role === "coordinator")?.member_id ?? ""
              }
              selectedTeamWorkerCount={selectedTeamWorkerCount}
              teams={teams}
              selectedTeam={selectedTeam}
              selectedTeamId={effectiveSelectedTeamId}
              selectedTeamRuntimeStatus={selectedTeamRuntimeStatus}
              selectedTeamHasConfiguredMembers={selectedTeamHasConfiguredMembers}
              teamMemberSummaryByTeamId={teamMemberSummaryByTeamId}
              selectedTeamMemberLiveStates={selectedTeamMemberLiveStates}
              channelItems={channelItems}
              workspaceTasks={workspaceTasks}
              routeChannelId={routeChannelId}
              focusedAgentMemberId={focusedAgentMemberId}
              routeWorkspaceLens={routeWorkspaceLens}
              tab={tab}
              navigateToTeamDetail={onSelectSidebarTeam}
              navigateToTeamSelector={navigateToTeamSelector}
              onSelectChannel={onSelectSidebarChannel}
              onCreateChannel={onCreateSidebarChannel}
              onDeleteChannel={onDeleteSidebarChannel}
              creatingChannel={busy === "create-team-channel"}
              deletingChannelId={deletingChannelId}
              onSelectKanbanSubject={onSelectKanbanSubject}
              onSelectConversationSubject={onSelectConversationSubject}
              onSelectAgentWorkspace={onSelectAgentWorkspace}
              onOpenTeamMemberForge={openTeamMemberForgeModal}
              onOpenTeamMemberCopyExisting={openCopyExistingAgentModal}
              teamMemberForgeLabel={teamMemberForgeLabel}
              teamMemberCopyExistingLabel={teamMemberCopyExistingLabel}
              onStartTeamRuntime={onStartTeamRuntime}
              onStopTeamRuntime={onStopTeamRuntime}
              onOpenMachines={() => navigateToPath(buildWorkspaceNodePath())}
              currentMachineId={focusedMemberTargetNodeId}
              onOpenCurrentMachine={() =>
                navigateToPath(buildWorkspaceNodePath(focusedMemberTargetNodeId))
              }
            />
          ) : null}
          {showWorkbenchPane ? (
            <TeamWorkspaceProvider value={teamWorkspaceContext}>
              <TeamWorkbenchContainer />
            </TeamWorkspaceProvider>
          ) : null}
        </div>
      )}
    </WorkspaceShell>
  );
}
