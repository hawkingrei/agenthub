import React, { Suspense, useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { Alert } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { ActionButton, IconButton } from "../ui/primitives";
import {
  deriveConnectionBadge,
  getNavigatorOnline,
  type SseConnectionState,
} from "../connection_status";
import {
  AGENT_SOURCE_TEAM_FORGE,
  AgentDiscoveryCardRecord,
  AgentRecord,
  AgentEvent,
  api,
  getApiErrorStatus,
  getTeamStepRuntimeHandleId,
  TeamConversationMessageRecord,
  TeamActorMessageRecord,
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
  DEFAULT_AGENT_PRESET_ID,
  getAgentPreset,
  type AgentPresetId,
} from "../agent_presets";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";
import {
  normalizeWorkdirInput,
  resolveWorkdirForModeChange,
} from "../worktree_defaults";
import { TeamEventsPanel } from "./team_events_panel";
import { TeamMailboxPanel } from "./team_mailbox_panel";
import { TeamMemberConsolePanel } from "./team_member_console_panel";
import { normalizeTeamMemberLifecycle } from "./team_member_status_strip";
import { TeamConversationPanel } from "./team_conversation_panel";
import { TeamTasksPanel } from "./team_tasks_panel";
import { TeamOverviewPanel } from "./team_overview_panel";
import { TeamRunPanel } from "./team_run_panel";
import { TeamSetupPanel } from "./team_setup_panel";
import { TeamSidebar } from "./team_sidebar";
import { TeamStepsPanel } from "./team_steps_panel";
import {
  TeamCreateDialog,
  TeamEditMemberDialog,
  TeamForgeAgentDialog,
} from "./team/team_management_modals";
import {
  TeamDebugToolsHeader,
  TeamRunOpsPanel,
  TeamRunRequiredPanel,
  type TeamDebugTag,
} from "./team/team_debug_panels";
import { TeamPageHeader } from "./team/team_page_header";
import { TeamSelectorPanel } from "./team/team_selector_panel";
import { TeamWorkspaceHeader } from "./team/team_workspace_header";
import {
  TeamLoadingPanel,
  TeamUnavailablePanel,
} from "./team_workspace_state_panel";
import {
  appendTeamMemberToSpec,
  buildTeamMemberDraftFromSpec,
  buildEmptyTeamSpec,
  buildLeaderForgeDefaultWorkdir,
  formatTeamForgeWorktreeError,
  parseErrorMessage,
  teamSpecHasConfiguredMembers,
  teamSpecHasLeader,
  updateTeamMemberProfileInSpec,
  type TeamMemberProfileDraft,
} from "./team/create_helpers";
import {
  clearTeamCreateDraft,
  loadTeamCreateDraft,
  persistTeamCreateDraft,
} from "./team/create_draft_storage";
import {
  resolveInitialTeamMemberRole,
  resolveTeamForgeDefaults,
  resolveTeamMemberRoleOptions,
  resolveTeamMemberRoleProfile,
  type TeamMemberRole,
} from "./team/forge_helpers";
import {
  MailboxTemplateKey,
  buildMailboxConversationKey,
  buildMailboxPayloadTemplate,
  createDisplayNameLookup,
  countUnreadConversationMessages,
  mergeMailboxMessages,
  resolveConversationMaxMessageId,
  resolveMailboxChatActors,
  selectMailboxConversation,
} from "./team/mailbox_helpers";
import {
  EMPTY_TEAM_PROMPT_DEFAULTS,
  TeamMemberAgentStatus,
  TeamMemberAgentStatusSummary,
  backfillEmptyWorkerDraftPrompts,
  buildTeamMemberLiveStates,
  parseTeamSpecMembers,
  resolveTeamPromptForRole,
  resolveTeamMemberAgentStatuses,
  summarizeTeamMemberAgentStatuses,
} from "./team/member_helpers";
import {
  DEFAULT_TEAM_THREAD_TITLE,
  formatTs,
  isSharedThreadTask,
  listTeamWorkspaceTasks,
  resolveAgentWorkspaceStatusView,
  resolveTeamPageNotice,
  resolveSelectedAgentWorkspaceLabel,
  resolveSelectedConversationTask,
  resolveSelectedTeamTask,
  shouldClearSelectedConversationTask,
  resolveTaskConversationMemberIds,
  removeTeamMemberLookupEntry,
  resolveTeamMemberAgentControlState,
  resolveTeamRuntimeControlTone,
  resolveTeamRuntimeStatus,
  sortTasksByActivity,
  toPrettyJson,
  updateCachedTeamRuntimeStatus,
  upsertRun,
} from "./team/page_helpers";
import {
  selectTeamPreviewEvents,
  type TeamRunStatusFilter,
} from "./team/run_helpers";
import { useTeamActions } from "./team/use_team_actions";
import { useTeamMailboxActions } from "./team/use_team_mailbox_actions";
import { useTeamConversationActions } from "./team/use_team_conversation_actions";
import { useTeamConversationEffects } from "./team/use_team_conversation_effects";
import { useTeamMemberAcpEffects } from "./team/use_team_member_acp_effects";
import { useTeamMemberAgentBackfillEffect } from "./team/use_team_member_agent_backfill_effect";
import { useTeamMailboxLifecycleEffects } from "./team/use_team_mailbox_lifecycle_effects";
import { useTeamTaskEffects } from "./team/use_team_task_effects";
import { useTeamRuntimeEffects } from "./team/use_team_runtime_effects";
import { useTeamRunLifecycleEffects } from "./team/use_team_run_lifecycle_effects";
import { useTeamStepActions } from "./team/use_team_step_actions";
import {
  DEFAULT_TEAM_CONTROL_STATE,
  DEFAULT_TEAM_MAILBOX_STATE,
  DEFAULT_TEAM_RUN_BROWSER_STATE,
  DEFAULT_TEAM_UI_STATE,
  DEFAULT_WORKTREE_ROOT,
  MAILBOX_TEMPLATE_OPTIONS,
  TEAM_TAB_ITEMS,
  TEAM_RUN_STATUS_FILTER_OPTIONS,
  tabRequiresActiveRun,
  createInitialTeamCreateState,
  reduceTeamControlState,
  reduceTeamCreateState,
  reduceTeamMailboxState,
  reduceTeamUiState,
  resolveUpdater,
  type TeamTab,
  type StepAction,
  type TeamControlState,
  type TeamCreateState,
  type TeamMailboxState,
  type TeamRunBrowserState,
} from "./team/state";
import {
  TEAM_DEBUG_TABS_CLASS,
  TEAM_DEBUG_TAB_ACTIVE_CLASS,
  TEAM_DEBUG_TAB_IDLE_CLASS,
  TEAM_PAGE_ROOT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_SECTION_BODY_TEXT_CLASS,
  TEAM_SECTION_CARD_CLASS,
  TEAM_SECTION_HEADING_CLASS,
  TEAM_SECTION_HINT_TEXT_CLASS,
  TEAM_SECTION_TITLE_CLASS,
  TEAM_WORKBENCH_HEADER_ICON_BUTTON_CLASS,
  TEAM_WORKBENCH_HEADER_SHELL_CLASS,
  TEAM_SOFT_CHROME_SHADOW_CLASS,
  TEAM_WORKBENCH_HEADER_STATUS_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS,
  TEAM_WORKBENCH_PANEL_CLASS,
  TEAM_WORKBENCH_WORKSPACE_SHELL_CLASS,
} from "../ui/tailwind_classes";

const LazyTeamMemberAcpPanel = React.lazy(async () => {
  const module = await import("./team_member_acp_panel");
  return { default: module.TeamMemberAcpPanel };
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
  DEFAULT_TEAM_LEADER_SKILLS,
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
  defaultWorktreeRoot?: string | null;
};

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

export function buildTeamDetailPath(teamId: string): string {
  return `/teams/${encodeURIComponent(teamId)}`;
}

function navigateTeamRoute(pathname: string): void {
  if (location.pathname === pathname) {
    return;
  }
  window.history.pushState({}, "", pathname);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

const TEAM_PRIMARY_WORKSPACE_TABS = new Set<TeamTab>(["conversation", "tasks"]);
const TEAM_WORKFLOW_TAB_ITEMS: ReadonlyArray<{ value: TeamTab; label: string }> = [
  { value: "conversation", label: "# all" },
  { value: "tasks", label: "Kanban" },
];
const TEAM_AGENT_WORKSPACE_TABS = new Set<TeamTab>(["agent_acp", "member_console"]);
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

export function isCurrentTeamScopedRequest(
  current: { teamId: string; requestSeq: number },
  teamId: string,
  requestSeq: number
): boolean {
  return Boolean(teamId) && current.teamId === teamId && current.requestSeq === requestSeq;
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

const panelSecondaryButtonClassName = TEAM_PANEL_SECONDARY_BUTTON_CLASS;
const teamSectionCardClassName = TEAM_SECTION_CARD_CLASS;
const teamSectionHeadingClassName = TEAM_SECTION_HEADING_CLASS;
const teamSectionTitleClassName = TEAM_SECTION_TITLE_CLASS;
const teamSectionBodyTextClassName = TEAM_SECTION_BODY_TEXT_CLASS;
const teamSectionHintTextClassName = TEAM_SECTION_HINT_TEXT_CLASS;
const teamDebugTabsClassName = TEAM_DEBUG_TABS_CLASS;
const teamDebugTabActiveClassName = TEAM_DEBUG_TAB_ACTIVE_CLASS;
const teamDebugTabIdleClassName = TEAM_DEBUG_TAB_IDLE_CLASS;
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
const workspaceNoticeDotBaseClassName =
  "inline-flex h-2 w-2 shrink-0 rounded-full";
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
const teamWorkbenchHeaderShellClassName = TEAM_WORKBENCH_HEADER_SHELL_CLASS;
const teamWorkbenchHeaderIconButtonClassName = TEAM_WORKBENCH_HEADER_ICON_BUTTON_CLASS;
const teamWorkbenchHeaderStatusClassName = TEAM_WORKBENCH_HEADER_STATUS_CLASS;
const teamWorkbenchDetailLayoutCollapsedClassName =
  "teams-layout grid min-h-0 flex-1 gap-3 lg:grid-cols-[minmax(0,1fr)] bg-white";
const teamWorkbenchDetailLayoutExpandedClassName =
  "teams-layout grid min-h-0 flex-1 gap-3 lg:grid-cols-[minmax(240px,280px)_minmax(0,1fr)] bg-white";
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
  const isSelectorRoute = routeTeamId == null;
  const routeDefaultWorktreeRoot = React.useMemo(() => {
    const normalized = normalizeWorkdirInput(props.defaultWorktreeRoot ?? "");
    return normalized || DEFAULT_WORKTREE_ROOT;
  }, [props.defaultWorktreeRoot]);
  const isCompactWorkbench = useMediaQuery("(max-width: 1023px)");
  const [error, setError] = useState<string | null>(null);
  const [warning, setWarning] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [networkOnline, setNetworkOnline] = useState<boolean>(getNavigatorOnline);
  const [teamRuntimeByTeamId, setTeamRuntimeByTeamId] = useState<Record<string, TeamRuntimeRecord>>(
    {}
  );
  const [teamsSidebarCollapsed, setTeamsSidebarCollapsed] = useState(false);
  const [workspaceDetailsOpen, setWorkspaceDetailsOpen] = useState(false);
  const [teamDebugTag, setTeamDebugTag] = useState<TeamDebugTag>("run_ops");
  const [conversationSseState, setConversationSseState] = useState<SseConnectionState>("idle");
  const mobileRouteTeamIdRef = useRef<string | null>(null);
  const previousCompactWorkbenchRef = useRef<boolean>(isCompactWorkbench);
  useEffect(() => {
    document.body.classList.add("teams-page");
    return () => {
      document.body.classList.remove("teams-page");
    };
  }, []);
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onOnline = () => {
      setNetworkOnline(true);
    };
    const onOffline = () => {
      setNetworkOnline(false);
    };
    window.addEventListener("online", onOnline);
    window.addEventListener("offline", onOffline);
    return () => {
      window.removeEventListener("online", onOnline);
      window.removeEventListener("offline", onOffline);
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
  const navigateToTeamSelector = useCallback(() => {
    navigateTeamRoute("/teams");
  }, []);

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
    [isSelectorRoute, routeTeamId]
  );
  useEffect(() => {
    if (isSelectorRoute && !wasSelectorRouteRef.current) {
      setTeamSelectorFilter("");
    }
    wasSelectorRouteRef.current = isSelectorRoute;
  }, [isSelectorRoute]);

  const [teamCreateState, dispatchTeamCreate] = useReducer(
    reduceTeamCreateState,
    undefined,
    createInitialTeamCreateState
  );
  const createDraftPersistErrorRef = useRef<string | null>(null);
  const newTeamName = teamCreateState.newTeamName;
  const newTeamDescription = teamCreateState.newTeamDescription;
  const showCreateTeamModal = teamCreateState.showCreateTeamModal;
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
  const [forgeDefaultWorktreeRoot, setForgeDefaultWorktreeRoot] = useState(
    DEFAULT_WORKTREE_ROOT
  );
  const [teamPromptDefaults, setTeamPromptDefaults] = useState(EMPTY_TEAM_PROMPT_DEFAULTS);
  const [teamMemberDraft, setTeamMemberDraft] = useState<TeamMemberProfileDraft | null>(null);
  const [teamMemberEditDraft, setTeamMemberEditDraft] =
    useState<TeamMemberProfileDraft | null>(null);
  const [showTeamMemberEditModal, setShowTeamMemberEditModal] = useState(false);
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
  const setShowCreateTeamModal = useCallback(
    (next: boolean) => patchTeamCreate({ showCreateTeamModal: next }),
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
  const patchTeamMemberDraft = useCallback((patch: Partial<TeamMemberProfileDraft>) => {
    setTeamMemberDraft((prev) => (prev ? { ...prev, ...patch } : prev));
  }, []);
  const patchTeamMemberEditDraft = useCallback((patch: Partial<TeamMemberProfileDraft>) => {
    setTeamMemberEditDraft((prev) => (prev ? { ...prev, ...patch } : prev));
  }, []);
  const setForgeAgentPresetId = useCallback(
    (next: AgentPresetId) => {
      patchTeamCreate({ forgeAgentPresetId: next });
      patchTeamMemberDraft({ model: next });
    },
    [patchTeamCreate, patchTeamMemberDraft]
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
  const [taskList, setTaskList] = useState<TeamTaskRecord[]>([]);
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
  const [compilePreviewContextId, setCompilePreviewContextId] = useState("");
  const [compiledRunPreview, setCompiledRunPreview] =
    useState<TeamTaskRunCompilePreviewRecord | null>(null);

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
  const [focusedAgentMemberId, setFocusedAgentMemberId] = useState("");
  const [teamCatalogSettled, setTeamCatalogSettled] = useState(() => isSelectorRoute);
  const handleTeamsRefreshSettled = useCallback(() => {
    setTeamCatalogSettled(true);
  }, []);

  const selectedTeam = useMemo(
    () => teams.find((team) => team.id === effectiveSelectedTeamId) ?? null,
    [effectiveSelectedTeamId, teams]
  );
  useEffect(() => {
    if (isSelectorRoute || teams.length > 0) {
      setTeamCatalogSettled(true);
    }
  }, [isSelectorRoute, teams.length]);
  useEffect(() => {
    setSelectedTeamId(routeTeamId);
  }, [routeTeamId]);
  useEffect(() => {
    if (routeTeamId == null) {
      setTeamCatalogSettled(true);
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
  const teamSpecMemberIds = useMemo(() => {
    const ids = new Set<string>();
    for (const team of teams) {
      for (const member of parseTeamSpecMembers(team.spec)) {
        ids.add(member.member_id);
      }
    }
    return [...ids];
  }, [teams]);
  useTeamMemberAgentBackfillEffect({
    token: props.token,
    agents,
    teamSpecMemberIds,
    teamMemberAgentsById,
    setTeamMemberAgentsById,
  });
  const teamMemberStatusByTeamId = useMemo(() => {
    const next = new Map<string, TeamMemberAgentStatus[]>();
    for (const team of teams) {
      next.set(
        team.id,
        resolveTeamMemberAgentStatuses(
          team.spec,
          agents,
          teamMemberAgentsById,
          teamRuntimeByTeamId[team.id]?.members
        )
      );
    }
    return next;
  }, [agents, teamMemberAgentsById, teamRuntimeByTeamId, teams]);
  const teamMemberSummaryByTeamId = useMemo(() => {
    const next = new Map<string, TeamMemberAgentStatusSummary>();
    for (const team of teams) {
      const members = teamMemberStatusByTeamId.get(team.id) ?? [];
      next.set(team.id, summarizeTeamMemberAgentStatuses(members));
    }
    return next;
  }, [teamMemberStatusByTeamId, teams]);
  const deferredTeamSelectorFilter = React.useDeferredValue(teamSelectorFilter);
  const normalizedTeamSelectorFilter = deferredTeamSelectorFilter.trim().toLowerCase();
  const selectorVisibleTeams = useMemo(() => {
    if (!normalizedTeamSelectorFilter) {
      return teams;
    }
    return teams.filter((team) => {
      const name = team.name.toLowerCase();
      const id = team.id.toLowerCase();
      return name.includes(normalizedTeamSelectorFilter) || id.includes(normalizedTeamSelectorFilter);
    });
  }, [normalizedTeamSelectorFilter, teams]);
  const selectorTeamItems = useMemo(
    () =>
      selectorVisibleTeams.map((team) => {
        const summary = teamMemberSummaryByTeamId.get(team.id);
        const runtime = teamRuntimeByTeamId[team.id] ?? null;
        const runtimeStatus = resolveTeamRuntimeStatus(summary, runtime);
        return {
          id: team.id,
          name: team.name,
          description: team.description?.trim() || "No mission summary yet.",
          summary: summary
            ? `${summary.total} members · ${summary.active} active${
                summary.inactive > 0 ? ` · ${summary.inactive} idle` : ""
              }${summary.missing > 0 ? ` · ${summary.missing} missing` : ""}`
            : "No agents configured yet",
          runtimeLabel: runtimeStatus.label,
        };
      }),
    [selectorVisibleTeams, teamMemberSummaryByTeamId, teamRuntimeByTeamId]
  );
  const selectedTeamMemberStatuses = useMemo(() => {
    if (!selectedTeam) {
      return [];
    }
    return teamMemberStatusByTeamId.get(selectedTeam.id) ?? [];
  }, [selectedTeam, teamMemberStatusByTeamId]);
  const selectedTeamSnapshotMembers = useMemo(() => {
    if (!selectedTeam || !snapshot) {
      return undefined;
    }
    if (snapshot.team.id !== selectedTeam.id) {
      return undefined;
    }
    return snapshot.members;
  }, [selectedTeam, snapshot]);
  const selectedTeamMemberLiveStates = useMemo(
    () =>
      buildTeamMemberLiveStates(selectedTeamMemberStatuses, selectedTeamSnapshotMembers),
    [selectedTeamMemberStatuses, selectedTeamSnapshotMembers]
  );
  const selectedTeamMemberSummary = useMemo(() => {
    if (!selectedTeam) {
      return null;
    }
    return teamMemberSummaryByTeamId.get(selectedTeam.id) ?? null;
  }, [selectedTeam, teamMemberSummaryByTeamId]);
  const selectedTeamRuntime = useMemo(() => {
    if (!selectedTeam) {
      return null;
    }
    return teamRuntimeByTeamId[selectedTeam.id] ?? null;
  }, [selectedTeam, teamRuntimeByTeamId]);
  const selectedTeamRuntimeStatus = useMemo(
    () => resolveTeamRuntimeStatus(selectedTeamMemberSummary, selectedTeamRuntime),
    [selectedTeamMemberSummary, selectedTeamRuntime]
  );
  const selectedTeamRuntimeControlTone = useMemo(
    () => resolveTeamRuntimeControlTone(selectedTeamRuntimeStatus.status),
    [selectedTeamRuntimeStatus.status]
  );
  const selectedTeamMembers = useMemo(
    () => (selectedTeam ? parseTeamSpecMembers(selectedTeam.spec) : []),
    [selectedTeam]
  );
  const selectedTeamHasConfiguredMembers = useMemo(
    () => (selectedTeam ? teamSpecHasConfiguredMembers(selectedTeam.spec) : false),
    [selectedTeam]
  );
  const shouldWatchSelectedTeamRuntime = useMemo(
    () =>
      selectedTeamHasConfiguredMembers &&
      (busy === "start-team" || selectedTeamRuntimeStatus.status !== "stopped"),
    [busy, selectedTeamHasConfiguredMembers, selectedTeamRuntimeStatus.status]
  );
  const selectedTeamHasLeader = useMemo(
    () => (selectedTeam ? teamSpecHasLeader(selectedTeam.spec) : false),
    [selectedTeam]
  );
  const selectedTeamWorkerCount = useMemo(
    () => selectedTeamMembers.filter((member) => member.role === "worker").length,
    [selectedTeamMembers]
  );
  const teamMemberRoleOptions = useMemo(
    () => resolveTeamMemberRoleOptions(selectedTeamHasLeader),
    [selectedTeamHasLeader]
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
  const teamMemberForgeLabel = "Add Agent";
  useEffect(() => {
    const memberId = selectedMemberId.trim();
    if (!memberId) {
      return;
    }
    const hasSelectedMember = selectedTeamMemberLiveStates.some(
      (member) => member.member_id === memberId
    );
    if (!hasSelectedMember) {
      setSelectedMemberId("");
    }
  }, [selectedMemberId, selectedTeamMemberLiveStates, setSelectedMemberId]);
  useEffect(() => {
    const memberId = focusedAgentMemberId.trim();
    if (!memberId) {
      return;
    }
    const hasFocusedMember = selectedTeamMemberLiveStates.some(
      (member) => member.member_id === memberId
    );
    if (!hasFocusedMember) {
      setFocusedAgentMemberId("");
    }
  }, [focusedAgentMemberId, selectedTeamMemberLiveStates]);
  useEffect(() => {
    if (tab !== "mailbox" || !snapshot || selectedMemberId.trim()) {
      return;
    }
    const defaultMailboxMemberId =
      snapshot.members.find((member) => member.member_id !== snapshot.leader_member_id)
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
  const selectedAgentWorkspaceMemberId = useMemo(
    () => selectedMemberId.trim() || focusedAgentMemberId.trim(),
    [focusedAgentMemberId, selectedMemberId]
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
  const selectedAgentWorkspaceSessionId = useMemo(() => {
    const snapshotSessionId = getTeamStepRuntimeHandleId(
      selectedAgentWorkspaceSnapshot?.latest_step
    );
    if (snapshotSessionId) {
      return snapshotSessionId;
    }
    const runtimeSessionId = selectedAgentWorkspaceRuntimeMember?.session_id?.trim();
    return runtimeSessionId && runtimeSessionId.length > 0 ? runtimeSessionId : null;
  }, [selectedAgentWorkspaceRuntimeMember, selectedAgentWorkspaceSnapshot]);
  const selectedAgentWorkspaceAgent = useMemo(() => {
    const memberId = selectedAgentWorkspaceMemberId.trim();
    if (!memberId) {
      return null;
    }
    return teamMemberAgentsById[memberId] ?? agents.find((agent) => agent.id === memberId) ?? null;
  }, [agents, selectedAgentWorkspaceMemberId, teamMemberAgentsById]);
  const selectedAgentWorkspaceAgentId = selectedAgentWorkspaceAgent?.id?.trim() ?? "";
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
    for (const actorId of chatMemberIds) {
      const actors = resolveMailboxChatActors(
        snapshot.leader_member_id,
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
      leaderPrompt: teamPromptDefaults.leader_prompt,
      showForgeAgentForm: initial.showForgeAgentForm,
      forgeAgentName: initial.forgeAgentName,
      forgeAgentWorkdir: initial.forgeAgentWorkdir,
      forgeAgentPresetId: initial.forgeAgentPresetId,
      forgeAgentWorktreeMode: initial.forgeAgentWorktreeMode,
      forgeAgentWorktreeRepo: initial.forgeAgentWorktreeRepo,
      forgeAgentWorktreeRef: initial.forgeAgentWorktreeRef,
      forgeAgentCodeMode: initial.forgeAgentCodeMode,
      forgeAgentWorktreeError: initial.forgeAgentWorktreeError,
      forgeAgentBusy: initial.forgeAgentBusy,
    });
    setTeamMemberDraft(null);
  }, [patchTeamCreate, teamPromptDefaults.leader_prompt]);

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
    if (!showCreateTeamModal || teamCreateState.leaderPrompt.trim()) {
      return;
    }
    if (!teamPromptDefaults.leader_prompt.trim()) {
      return;
    }
    patchTeamCreate({ leaderPrompt: teamPromptDefaults.leader_prompt });
  }, [
    patchTeamCreate,
    showCreateTeamModal,
    teamCreateState.leaderPrompt,
    teamPromptDefaults.leader_prompt,
  ]);

  useEffect(() => {
    if (!showCreateTeamModal) {
      return;
    }
    const nextWorkers = backfillEmptyWorkerDraftPrompts(
      teamCreateState.workers,
      teamPromptDefaults
    );
    if (nextWorkers === teamCreateState.workers) {
      return;
    }
    patchTeamCreate({ workers: nextWorkers });
  }, [
    patchTeamCreate,
    showCreateTeamModal,
    teamCreateState.workers,
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
    selectedMemberAgentId: selectedAgentWorkspaceAgentId || null,
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

  useTeamRunLifecycleEffects({
    selectedTeamId: effectiveSelectedTeamId,
    runStatusFilter,
    runs,
    activeRunIdForSelectedTeam,
    eventsAutoRefresh,
    tab,
    chatInboxActorId: chatActors.inboxActorId,
    refreshAgents,
    refreshTeams,
    refreshTeamRuns,
    refreshRun,
    refreshSteps,
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

  const openCreateTeamModal = useCallback(() => {
    const { draft: restoredDraft, error: restoreError } = loadTeamCreateDraft("wizard");
    setError(null);
    setWarning(null);
    if (restoreError) {
      setError(restoreError);
    }
    resetTeamDraft();
    if (restoredDraft) {
      patchTeamCreate({
        ...restoredDraft,
        leaderPrompt: restoredDraft.leaderPrompt || teamPromptDefaults.leader_prompt,
        workers: backfillEmptyWorkerDraftPrompts(
          restoredDraft.workers,
          teamPromptDefaults
        ),
        showCreateTeamModal: true,
        showForgeAgentForm: false,
        forgeAgentWorktreeError: null,
        forgeAgentBusy: false,
      });
      return;
    }
    setShowCreateTeamModal(true);
    setShowForgeAgentForm(false);
    setForgeAgentWorktreeError(null);
  }, [
    patchTeamCreate,
    resetTeamDraft,
    teamPromptDefaults,
    setError,
    setShowCreateTeamModal,
    setShowForgeAgentForm,
    setForgeAgentWorktreeError,
    setWarning,
  ]);

  const closeCreateTeamModal = useCallback(() => {
    if (busy === "create-team") {
      return;
    }
    setShowCreateTeamModal(false);
  }, [busy, setShowCreateTeamModal]);

  const openTeamMemberForgeModal = useCallback(() => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    const role = resolveInitialTeamMemberRole(selectedTeamHasLeader);
    const defaults = resolveTeamForgeDefaults({
      teamName: selectedTeam.name,
      teamSpec: selectedTeam.spec,
      role,
      workerCount: selectedTeamWorkerCount,
      defaultWorktreeRoot: forgeDefaultWorktreeRoot,
      agentPresetId: DEFAULT_AGENT_PRESET_ID,
      promptDefaults: teamPromptDefaults,
    });

    setError(null);
    setWarning(null);
    setTeamMemberDraft(defaults.draft);
    setShowForgeAgentForm(true);
    setForgeAgentName(defaults.agentName);
    setForgeAgentWorktreeMode(defaults.worktreeMode);
    setForgeAgentWorktreeRepo(defaults.worktreeRepo);
    setForgeAgentWorktreeRef(defaults.worktreeRef);
    setForgeAgentPresetId(DEFAULT_AGENT_PRESET_ID);
    setForgeAgentCodeMode(true);
    setForgeAgentWorktreeError(null);
    setForgeAgentWorkdir(defaults.agentWorkdir);
  }, [
    forgeDefaultWorktreeRoot,
    selectedTeam,
    selectedTeamHasLeader,
    selectedTeamWorkerCount,
    teamPromptDefaults,
    setError,
    setWarning,
    setShowForgeAgentForm,
    setForgeAgentCodeMode,
    setForgeAgentName,
    setForgeAgentPresetId,
    setForgeAgentWorkdir,
    setForgeAgentWorktreeError,
    setForgeAgentWorktreeMode,
    setForgeAgentWorktreeRef,
    setForgeAgentWorktreeRepo,
  ]);

  const handleTeamMemberRoleChange = useCallback(
    (nextRole: string) => {
      if (!selectedTeam) {
        return;
      }
      if (nextRole !== "leader" && nextRole !== "worker") {
        return;
      }
      const role = nextRole as TeamMemberRole;
      const roleOption = teamMemberRoleOptions.find((option) => option.value === role);
      if (!roleOption || roleOption.disabled) {
        return;
      }
      const defaults = resolveTeamForgeDefaults({
        teamName: selectedTeam.name,
        teamSpec: selectedTeam.spec,
        role,
        workerCount: selectedTeamWorkerCount,
        defaultWorktreeRoot: forgeDefaultWorktreeRoot,
        agentPresetId: forgeAgentPresetId,
        promptDefaults: teamPromptDefaults,
      });
      setError(null);
      setWarning(null);
      setTeamMemberDraft(defaults.draft);
      setForgeAgentName(defaults.agentName);
      setForgeAgentWorktreeMode(defaults.worktreeMode);
      setForgeAgentWorktreeRepo(defaults.worktreeRepo);
      setForgeAgentWorktreeRef(defaults.worktreeRef);
      setForgeAgentWorktreeError(null);
      setForgeAgentWorkdir(defaults.agentWorkdir);
    },
    [
      forgeAgentPresetId,
      forgeDefaultWorktreeRoot,
      selectedTeam,
      selectedTeamWorkerCount,
      teamPromptDefaults,
      setError,
      setWarning,
      setForgeAgentName,
      setForgeAgentWorkdir,
      setForgeAgentWorktreeError,
      setForgeAgentWorktreeMode,
      setForgeAgentWorktreeRef,
      setForgeAgentWorktreeRepo,
      teamMemberRoleOptions,
    ]
  );

  const closeTeamMemberForgeModal = useCallback(() => {
    if (forgeAgentBusy) {
      return;
    }
    setShowForgeAgentForm(false);
    setForgeAgentWorktreeError(null);
    setTeamMemberDraft(null);
  }, [forgeAgentBusy, setShowForgeAgentForm, setForgeAgentWorktreeError]);

  const openTeamMemberEditModal = useCallback(() => {
    if (!selectedTeam || !selectedAgentWorkspaceMemberId) {
      setError("Select an agent first");
      return;
    }
    const draft = buildTeamMemberDraftFromSpec(
      selectedTeam.spec,
      selectedAgentWorkspaceMemberId,
      teamMemberAgentsById[selectedAgentWorkspaceMemberId] ?? null,
      teamPromptDefaults
    );
    if (!draft) {
      setError("Unable to load the selected agent profile");
      return;
    }
    setError(null);
    setWarning(null);
    setTeamMemberEditDraft(draft);
    setShowTeamMemberEditModal(true);
  }, [
    teamPromptDefaults,
    selectedAgentWorkspaceMemberId,
    selectedTeam,
    setError,
    setWarning,
    teamMemberAgentsById,
  ]);

  const closeTeamMemberEditModal = useCallback(() => {
    if (busy === "save-team-member-profile") {
      return;
    }
    setShowTeamMemberEditModal(false);
    setTeamMemberEditDraft(null);
  }, [busy]);

  const refreshTeamRuntime = useCallback(
    async (teamId: string, options?: { apply?: boolean }) => {
      const runtime = await api.getTeamRuntime(props.token, teamId);
      if (options?.apply !== false) {
        setTeamRuntimeByTeamId((prev) => ({ ...prev, [teamId]: runtime }));
      }
      return runtime;
    },
    [props.token]
  );
  const applyOptimisticTeamRuntime = useCallback(
    (
      teamId: string,
      teamName: string,
      runtime: Awaited<ReturnType<typeof api.startTeam>>,
      memberStatuses: TeamMemberAgentStatus[]
    ) => {
      setTeamRuntimeByTeamId((prev) => {
        const previousRuntime = prev[teamId];
        const optimisticRuntime = updateCachedTeamRuntimeStatus(
          previousRuntime,
          teamId,
          teamName,
          runtime.status as TeamRuntimeRecord["status"],
          runtime.members,
          (sessionStatus) => {
            if (runtime.status !== "running") {
              return sessionStatus ?? undefined;
            }
            return "running";
          },
          memberStatuses
        );
        if (!optimisticRuntime) {
          return prev;
        }
        return {
          ...prev,
          [teamId]: optimisticRuntime,
        };
      });
    },
    []
  );

  const onCreateForgeAgent = async () => {
    if (forgeAgentBusy) {
      return;
    }
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    if (!teamMemberDraft) {
      setError("Open Add Agent first");
      return;
    }

    const isLeaderRole = teamMemberDraft.role === "leader";
    const effectiveWorktreeMode = isLeaderRole ? "use_existing" : forgeAgentWorktreeMode;
    const effectiveWorktreeRepo = isLeaderRole ? "" : forgeAgentWorktreeRepo.trim();
    const effectiveWorktreeRef = isLeaderRole ? "" : forgeAgentWorktreeRef.trim();
    const normalizedRoot =
      normalizeWorkdirInput(forgeDefaultWorktreeRoot) || DEFAULT_WORKTREE_ROOT;
    const name = forgeAgentName.trim() || "agent";
    const workdirInput = normalizeWorkdirInput(forgeAgentWorkdir);
    const workdir =
      isLeaderRole && !workdirInput
        ? buildLeaderForgeDefaultWorkdir(normalizedRoot, name)
        : workdirInput;
    const workdirPayload =
      effectiveWorktreeMode === "create_worktree" &&
      normalizedRoot &&
      workdir === normalizedRoot
        ? ""
        : workdir;

    if (!workdirPayload && effectiveWorktreeMode !== "create_worktree") {
      setError("Agent workdir is required");
      return;
    }
    if (effectiveWorktreeMode !== "use_existing" && !effectiveWorktreeRepo) {
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
        source: AGENT_SOURCE_TEAM_FORGE,
        worktree_mode: effectiveWorktreeMode,
        worktree_repo: effectiveWorktreeRepo || null,
        worktree_ref: effectiveWorktreeRef || null,
        code_mode: forgeAgentCodeMode,
      });
      const nextSpec = appendTeamMemberToSpec(
        selectedTeam.spec,
        { ...teamMemberDraft, member_id: created.id },
        created,
        teamPromptDefaults
      );
      const updated = await api.updateTeamSpec(props.token, selectedTeam.id, {
        spec: nextSpec,
        expected_updated_at: selectedTeam.updated_at,
      });
      setAgents((prev) => [created, ...prev.filter((agent) => agent.id !== created.id)]);
      setTeams((prev) =>
        [...prev.filter((team) => team.id !== updated.id), updated].sort((left, right) =>
          left.name.localeCompare(right.name)
        )
      );
      setSelectedTeamId(updated.id);
      setShowForgeAgentForm(false);
      setForgeAgentWorktreeError(null);
      setTeamMemberDraft(null);
      void refreshTeamRuntime(updated.id).catch(() => undefined);
    } catch (err) {
      const hint = formatTeamForgeWorktreeError(err);
      setForgeAgentWorktreeError(hint);
      setError(hint ?? parseErrorMessage(err));
    } finally {
      setForgeAgentBusy(false);
    }
  };

  const onSaveTeamMemberProfile = useCallback(async () => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    if (!teamMemberEditDraft) {
      setError("Open Edit Profile first");
      return;
    }
    setBusy("save-team-member-profile");
    setError(null);
    setWarning(null);
    try {
      const nextSpec = updateTeamMemberProfileInSpec(
        selectedTeam.spec,
        teamMemberEditDraft,
        teamPromptDefaults
      );
      const updated = await api.updateTeamSpec(props.token, selectedTeam.id, {
        spec: nextSpec,
        expected_updated_at: selectedTeam.updated_at,
      });
      const idleSeconds = teamMemberEditDraft.agent_loop_idle_seconds.trim();
      const parsedIdleSeconds = Number.parseInt(idleSeconds, 10);
      const loopPayload = {
        enabled: teamMemberEditDraft.agent_loop_enabled,
        idle_seconds:
          teamMemberEditDraft.agent_loop_enabled &&
          idleSeconds !== "" &&
          Number.isFinite(parsedIdleSeconds)
            ? parsedIdleSeconds
            : null,
        prompt:
          teamMemberEditDraft.agent_loop_enabled && teamMemberEditDraft.agent_loop_prompt.trim()
            ? teamMemberEditDraft.agent_loop_prompt.trim()
            : null,
      };
      try {
        await api.setAgentLoop(props.token, teamMemberEditDraft.member_id, loopPayload);
        setAgents((prev) =>
          prev.map((agent) =>
            agent.id === teamMemberEditDraft.member_id
              ? {
                  ...agent,
                  agent_loop_enabled: loopPayload.enabled,
                  agent_loop_idle_seconds: loopPayload.idle_seconds,
                  agent_loop_prompt: loopPayload.prompt,
                }
              : agent
          )
        );
        setTeamMemberAgentsById((prev) => ({
          ...prev,
          [teamMemberEditDraft.member_id]: prev[teamMemberEditDraft.member_id]
            ? {
                ...prev[teamMemberEditDraft.member_id],
                agent_loop_enabled: loopPayload.enabled,
                agent_loop_idle_seconds: loopPayload.idle_seconds,
                agent_loop_prompt: loopPayload.prompt,
              }
            : prev[teamMemberEditDraft.member_id],
        }));
      } catch (loopErr) {
        setWarning(
          `Agent loop settings were not applied: ${parseErrorMessage(loopErr)}`
        );
      }
      setTeams((prev) =>
        [...prev.filter((team) => team.id !== updated.id), updated].sort((left, right) =>
          left.name.localeCompare(right.name)
        )
      );
      setSelectedTeamId(updated.id);
      setShowTeamMemberEditModal(false);
      setTeamMemberEditDraft(null);
      void refreshTeamRuntime(updated.id).catch(() => undefined);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    props.token,
    refreshTeamRuntime,
    selectedTeam,
    setBusy,
    setError,
    setWarning,
    teamPromptDefaults,
    teamMemberEditDraft,
  ]);

  const onCreateTeam = async () => {
    const name = newTeamName.trim();
    if (!name) {
      setError("Team name is required");
      return;
    }
    setBusy("create-team");
    setError(null);
    setWarning(null);
    try {
      const created = await api.createTeam(props.token, {
        name,
        description: newTeamDescription.trim() || undefined,
        spec: buildEmptyTeamSpec(),
      });
      setTeams((prev) => [...prev, created].sort((a, b) => a.name.localeCompare(b.name)));
      clearTeamCreateDraft();
      resetTeamDraft();
      setShowCreateTeamModal(false);
      navigateToTeamDetail(created.id);
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
      setSelectedTeamId((current) => (current === selectedTeam.id ? null : current));
      setActiveRunId((current) =>
        current && remainingRuns.some((run) => run.id === current) ? current : null
      );
      setRunLookupId("");
      setTeamSelectorFilter("");
      navigateToTeamSelector();
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

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

  const resolvedSelectedConversationTaskId = selectedConversationTaskId.trim();
  const selectedConversation = useMemo(() => {
    if (!effectiveSelectedTeamId) {
      return null;
    }
    return resolveSelectedConversationTask({
      taskList,
      selectedTaskId: resolvedSelectedConversationTaskId,
      sharedConversation,
      fallbackTask: selectedConversationDetail?.task ?? null,
    });
  }, [
    effectiveSelectedTeamId,
    resolvedSelectedConversationTaskId,
    selectedConversationDetail?.task,
    sharedConversation,
    taskList,
  ]);
  const selectedConversationLatestRun = useMemo(() => {
    if (!resolvedSelectedConversationTaskId) {
      return sharedConversationLatestRun;
    }
    if (sharedConversation?.id === resolvedSelectedConversationTaskId) {
      return sharedConversationLatestRun;
    }
    return selectedConversationDetail?.latest_run ?? null;
  }, [
    resolvedSelectedConversationTaskId,
    selectedConversationDetail,
    sharedConversation?.id,
    sharedConversationLatestRun,
  ]);
  const selectedConversationId = selectedConversation?.id ?? null;
  const hasConversationStreamTarget = Boolean(
    eventsAutoRefresh &&
      effectiveSelectedTeamId &&
      (selectedConversationId ?? "").trim()
  );
  const connectionBadge = useMemo(
    () =>
      deriveConnectionBadge(
        networkOnline,
        hasConversationStreamTarget,
        conversationSseState
      ),
    [conversationSseState, hasConversationStreamTarget, networkOnline]
  );
  const workspaceTasks = useMemo(() => {
    if (!effectiveSelectedTeamId) {
      return [];
    }
    return listTeamWorkspaceTasks(taskList, effectiveSelectedTeamId);
  }, [effectiveSelectedTeamId, taskList]);

  const selectedTask = useMemo(() => {
    if (!effectiveSelectedTeamId) {
      return null;
    }
    return resolveSelectedTeamTask(taskList, selectedTaskId, effectiveSelectedTeamId);
  }, [effectiveSelectedTeamId, selectedTaskId, taskList]);

  const refreshTasks = useCallback(
    async (teamId: string) => {
      setTasksLoading(true);
      try {
        const list = await api.listTeamTasks(props.token, teamId, 100, {
          include_shared_thread: true,
        });
        const sorted = sortTasksByActivity(list);
        setTaskList(sorted);
        setSelectedTaskId((prev) => {
          return resolveSelectedTeamTask(sorted, prev, teamId)?.id ?? "";
        });
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
        setTasksLoading(false);
      }
    },
    [props.token]
  );

  const refreshSharedConversation = useCallback(
    async (teamId: string) => {
      const normalizedTeamId = teamId.trim();
      const requestSeq = sharedConversationRequestScopeRef.current.requestSeq;
      const isCurrentRequest = () =>
        isCurrentTeamScopedRequest(
          sharedConversationRequestScopeRef.current,
          normalizedTeamId,
          requestSeq
        );
      try {
        const detail = await api.getTeamSharedThread(props.token, normalizedTeamId);
        if (!isCurrentRequest()) {
          return;
        }
        setSharedConversation(detail.task);
        setSharedConversationLatestRun(detail.latest_run ?? null);
      } catch (err) {
        if (!isCurrentRequest()) {
          return;
        }
        if (getApiErrorStatus(err) === 404) {
          setSharedConversation(null);
          setSharedConversationLatestRun(null);
          setTaskMessages([]);
          setConversationMailboxMessages([]);
          return;
        }
        setError(parseErrorMessage(err));
      }
    },
    [props.token, setError, setConversationMailboxMessages, setTaskMessages]
  );

  useEffect(() => {
    if (!effectiveSelectedTeamId) {
      return;
    }
    const taskId = resolvedSelectedConversationTaskId;
    if (!taskId) {
      setSelectedConversationDetail(null);
      return;
    }
    if (sharedConversation?.id === taskId) {
      setSelectedConversationDetail(null);
      return;
    }
    let active = true;
    void api
      .getTeamTask(props.token, effectiveSelectedTeamId, taskId)
      .then((detail) => {
        if (!active) {
          return;
        }
        setSelectedConversationDetail(detail);
      })
      .catch((err) => {
        if (!active) {
          return;
        }
        setSelectedConversationDetail(null);
        setError(parseErrorMessage(err));
      });
    return () => {
      active = false;
    };
  }, [
    effectiveSelectedTeamId,
    props.token,
    resolvedSelectedConversationTaskId,
    setError,
    sharedConversation?.id,
  ]);

  useEffect(() => {
    const shouldClearSelection = shouldClearSelectedConversationTask({
      selectedConversationTaskId: resolvedSelectedConversationTaskId,
      sharedConversationTaskId: sharedConversation?.id ?? null,
      taskList,
      selectedConversationDetailPresent: Boolean(selectedConversationDetail),
      tasksLoading,
    });
    if (!shouldClearSelection) {
      return;
    }
    setSelectedConversationTaskId("");
    setSelectedConversationDetail(null);
  }, [
    resolvedSelectedConversationTaskId,
    selectedConversationDetail,
    sharedConversation?.id,
    taskList,
    tasksLoading,
  ]);

  useEffect(() => {
    setCompiledRunPreview(null);
    setCompilePreviewContextId("");
  }, [selectedTaskId, effectiveSelectedTeamId]);

  useEffect(() => {
    if (!effectiveSelectedTeamId) {
      return;
    }
    void refreshTasks(effectiveSelectedTeamId);
    void refreshSharedConversation(effectiveSelectedTeamId);
  }, [effectiveSelectedTeamId, refreshSharedConversation, refreshTasks]);

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

  useTeamConversationEffects({
    token: props.token,
    selectedTeamId: effectiveSelectedTeamId,
    selectedConversationId,
    tab,
    eventsAutoRefresh,
    refreshTaskMessages,
    setTaskMessages,
    setConversationMailboxMessages,
    onSseStateChange: setConversationSseState,
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

  useTeamMemberAcpEffects({
    token: props.token,
    selectedAgentId: selectedAgentWorkspaceAgentId,
    selectedSessionId: selectedAgentWorkspaceSessionId,
    tab,
    eventsAutoRefresh,
    loadMemberEvents,
    setMemberEvents,
    setMemberEventsHasMore,
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

  const onOpenMailboxForMember = useCallback((memberId: string) => {
    setSelectedMemberId(memberId);
    setFocusedAgentMemberId("");
    setTab("mailbox");
    if (isCompactWorkbench) {
      setTeamsSidebarCollapsed(true);
    }
  }, [isCompactWorkbench, setSelectedMemberId, setTab]);
  const onSelectConversationSubject = useCallback((taskId?: string | null) => {
    setFocusedAgentMemberId("");
    setSelectedConversationTaskId(typeof taskId === "string" ? taskId.trim() : "");
    setTab("conversation");
    if (isCompactWorkbench) {
      setTeamsSidebarCollapsed(true);
    }
  }, [isCompactWorkbench, setTab]);
  const onSelectKanbanSubject = useCallback(() => {
    setFocusedAgentMemberId("");
    setTab("tasks");
    if (isCompactWorkbench) {
      setTeamsSidebarCollapsed(true);
    }
  }, [isCompactWorkbench, setTab]);
  const onSelectAgentWorkspace = useCallback(
    (memberId: string, nextTab: TeamTab = "agent_acp") => {
      setSelectedMemberId(memberId);
      setFocusedAgentMemberId(memberId);
      setTab(nextTab);
      if (isCompactWorkbench) {
        setTeamsSidebarCollapsed(true);
      }
    },
    [isCompactWorkbench, setSelectedMemberId, setTab]
  );
  const onSelectUtilityWorkspace = useCallback(
    (nextTab: TeamTab) => {
      setFocusedAgentMemberId("");
      setTab(nextTab);
      if (isCompactWorkbench) {
        setTeamsSidebarCollapsed(true);
      }
    },
    [isCompactWorkbench, setTab]
  );
  const onSelectSidebarTeam = useCallback(
    (teamId: string) => {
      if (teamId !== effectiveSelectedTeamId) {
        navigateToTeamDetail(teamId);
      }
    },
    [effectiveSelectedTeamId, navigateToTeamDetail]
  );
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
  const tabNeedsActiveRun = tabRequiresActiveRun(tab);
  const showRunContextLoading = tab !== "runs" && tabNeedsActiveRun && runsLoading && !activeRunForSelectedTeam;
  const showNoActiveRunNotice = tab !== "runs" && tabNeedsActiveRun && !runsLoading && !activeRunForSelectedTeam;
  const selectedMemberLiveState = useMemo(
    () =>
      selectedTeamMemberLiveStates.find((member) => member.member_id === selectedMemberId) ?? null,
    [selectedMemberId, selectedTeamMemberLiveStates]
  );
  const selectedAgentWorkspaceLiveState = useMemo(
    () =>
      selectedTeamMemberLiveStates.find(
        (member) => member.member_id === selectedAgentWorkspaceMemberId
      ) ?? null,
    [selectedAgentWorkspaceMemberId, selectedTeamMemberLiveStates]
  );
  const selectedAgentLiveState = useMemo(
    () =>
      selectedTeamMemberLiveStates.find((member) => member.member_id === focusedAgentMemberId) ??
      null,
    [focusedAgentMemberId, selectedTeamMemberLiveStates]
  );
  const hasSelectedAgentContext = focusedAgentMemberId.trim().length > 0;
  const isAgentWorkspace =
    hasSelectedAgentContext && (TEAM_AGENT_WORKSPACE_TABS.has(tab) || tab === "mailbox");
  const workspaceAdvancedTabItems = (isAgentWorkspace
    ? TEAM_AGENT_ADVANCED_TAB_ITEMS
    : TEAM_UTILITY_ADVANCED_TAB_ITEMS
  ).filter((item) => props.developerMode || item.value !== "debug");
  const isAdvancedWorkspace = workspaceAdvancedTabItems.some((item) => item.value === tab);
  const showRunActionsInAdvanced = Boolean(activeRunForSelectedTeam && tab !== "runs");
  const workspaceEyebrow = null;
  const selectedAgentFallbackName = useMemo(() => {
    const memberId = focusedAgentMemberId.trim();
    if (!memberId) {
      return null;
    }
    return (
      teamMemberAgentsById[memberId]?.name?.trim() ??
      agents.find((agent) => agent.id === memberId)?.name?.trim() ??
      null
    );
  }, [agents, focusedAgentMemberId, teamMemberAgentsById]);
  const selectedAgentLabel = useMemo(
    () =>
      resolveSelectedAgentWorkspaceLabel(
        focusedAgentMemberId,
        selectedAgentLiveState,
        selectedAgentFallbackName
      ),
    [focusedAgentMemberId, selectedAgentFallbackName, selectedAgentLiveState]
  );
  const selectedAgentSpecDraft = useMemo(() => {
    if (!selectedTeam) {
      return null;
    }
    return buildTeamMemberDraftFromSpec(
      selectedTeam.spec,
      selectedAgentWorkspaceMemberId,
      selectedAgentWorkspaceMemberId
        ? teamMemberAgentsById[selectedAgentWorkspaceMemberId] ?? null
        : null,
      teamPromptDefaults
    );
  }, [selectedAgentWorkspaceMemberId, selectedTeam, teamMemberAgentsById, teamPromptDefaults]);
  const selectedAgentStatusView = useMemo(
    () => resolveAgentWorkspaceStatusView(selectedAgentLiveState),
    [selectedAgentLiveState]
  );
  const activeConversationTitle = useMemo(
    () => selectedConversation?.title?.trim() || DEFAULT_TEAM_THREAD_TITLE,
    [selectedConversation]
  );
  const selectedConversationIsShared = useMemo(
    () => (selectedConversation ? isSharedThreadTask(selectedConversation) : true),
    [selectedConversation]
  );
  const currentWorkspaceTabLabel = useMemo(
    () => TEAM_TAB_ITEMS.find((item) => item.value === tab)?.label ?? selectedTeam?.name ?? "Team",
    [selectedTeam?.name, tab]
  );
  const workspaceTitle = !selectedTeam
    ? "Team Workbench"
    : isAgentWorkspace
      ? selectedAgentLabel
    : tab === "conversation"
      ? selectedConversationIsShared
        ? `# ${activeConversationTitle}`
        : activeConversationTitle
    : tab === "tasks"
      ? "Kanban"
    : tab === "mailbox"
      ? selectedMemberLiveState
        ? selectedAgentLabel
        : "Execution Mailbox"
    : selectedMemberLiveState && isAgentWorkspace
      ? selectedAgentLabel
      : currentWorkspaceTabLabel;
  const workspaceDescription = !selectedTeam
    ? "Select a team from the left rail to start team conversations and supervise execution."
    : isAgentWorkspace
      ? null
    : tab === "conversation"
      ? selectedConversationIsShared
        ? "Shared channel for human requests, planning discussion, and team-visible progress updates."
        : "Task thread for the selected Team task. Use it for task-scoped follow-up and execution context."
    : tab === "tasks"
        ? "Canonical Kanban for leader-planned, system-managed Team tasks. Human task requests belong in # all."
      : tab === "mailbox"
        ? selectedMemberLiveState
          ? "Direct mailbox thread for the selected agent."
          : "Run-scoped mailbox delivery and direct member conversations."
      : tab === "runs"
        ? "Browse runs and choose the active execution context."
        : isAgentWorkspace
          ? "Direct thread for the selected agent."
          : "Operational views stay available without displacing the main thread.";
  const showWorkspaceRuntimeBadge =
    !isAgentWorkspace && !TEAM_PRIMARY_WORKSPACE_TABS.has(tab);
  const showDedicatedWorkspaceHeading =
    isAgentWorkspace || !selectedTeam || workspaceTitle !== selectedTeam.name;
  const workspaceMemberAvailability = useMemo(() => {
    if (selectedTeamMemberSummary) {
      return {
        online: selectedTeamMemberSummary.active,
        offline: selectedTeamMemberSummary.inactive,
        missing: selectedTeamMemberSummary.missing,
      };
    }
    let online = 0;
    let offline = 0;
    let missing = 0;
    for (const member of selectedTeamMemberLiveStates) {
      const lifecycle = normalizeTeamMemberLifecycle(member);
      if (lifecycle === "missing") {
        missing += 1;
      } else if (lifecycle === "stopped") {
        offline += 1;
      } else {
        online += 1;
      }
    }
    return { online, offline, missing };
  }, [selectedTeamMemberLiveStates, selectedTeamMemberSummary]);
  const workspaceNoticeText = useMemo(() => {
    if (isAgentWorkspace) {
      return null;
    }
    if (tab === "conversation" || tab === "tasks") {
      return null;
    }
    const runtimeLabel = selectedTeam ? selectedTeamRuntimeStatus.label : null;
    const runLabel = activeRunForSelectedTeam
      ? `run ${activeRunForSelectedTeam.status}`
      : "no active run";
    const rosterLabel = `${selectedTeamMemberLiveStates.length} members`;
    const availabilityLabel =
      workspaceMemberAvailability.missing > 0
        ? `${workspaceMemberAvailability.missing} missing`
        : workspaceMemberAvailability.offline > 0
          ? `${workspaceMemberAvailability.offline} offline`
          : `${workspaceMemberAvailability.online} online`;
    return [runtimeLabel, runLabel, rosterLabel, availabilityLabel]
      .filter((value): value is string => Boolean(value))
      .join(" · ");
  }, [
    activeRunForSelectedTeam,
    isAgentWorkspace,
    selectedTeam,
    selectedTeamMemberLiveStates.length,
    selectedTeamRuntimeStatus.label,
    tab,
    workspaceMemberAvailability,
  ]);
  const workspaceNoticeDotClassName = useMemo(() => {
    if (isAgentWorkspace) {
      if (selectedAgentStatusView.lifecycle === "missing") {
        return `${workspaceNoticeDotBaseClassName} bg-rose-500`;
      }
      if (selectedAgentStatusView.work === "blocked") {
        return `${workspaceNoticeDotBaseClassName} bg-rose-500`;
      }
      if (
        selectedAgentStatusView.lifecycle === "working" ||
        selectedAgentStatusView.work === "working" ||
        selectedAgentStatusView.work === "pending" ||
        selectedAgentStatusView.work === "done"
      ) {
        return `${workspaceNoticeDotBaseClassName} bg-emerald-500`;
      }
      if (selectedAgentStatusView.lifecycle === "stopped") {
        return `${workspaceNoticeDotBaseClassName} bg-slate-400`;
      }
    }
    if (workspaceMemberAvailability.missing > 0) {
      return `${workspaceNoticeDotBaseClassName} bg-rose-500`;
    }
    if (selectedTeamRuntimeStatus.status === "degraded") {
      return `${workspaceNoticeDotBaseClassName} bg-amber-500`;
    }
    if (selectedTeamRuntimeStatus.status === "stopped") {
      return `${workspaceNoticeDotBaseClassName} bg-slate-400`;
    }
    if (!activeRunForSelectedTeam) {
      return `${workspaceNoticeDotBaseClassName} bg-slate-400`;
    }
    if (activeRunForSelectedTeam.status === "working") {
      return `${workspaceNoticeDotBaseClassName} bg-emerald-500`;
    }
    if (activeRunForSelectedTeam.status === "completed") {
      return `${workspaceNoticeDotBaseClassName} bg-emerald-500`;
    }
    if (activeRunForSelectedTeam.status === "failed") {
      return `${workspaceNoticeDotBaseClassName} bg-rose-500`;
    }
    if (activeRunForSelectedTeam.status === "canceled") {
      return `${workspaceNoticeDotBaseClassName} bg-amber-500`;
    }
    return `${workspaceNoticeDotBaseClassName} bg-slate-400`;
  }, [
    activeRunForSelectedTeam,
    isAgentWorkspace,
    selectedAgentStatusView.lifecycle,
    selectedAgentStatusView.work,
    selectedTeamRuntimeStatus.status,
    workspaceMemberAvailability.missing,
  ]);
  const workspaceDetailItems = useMemo(
    () =>
      [
        `team=${selectedTeam?.id ?? "-"}`,
        `team_runtime=${selectedTeamRuntimeStatus.status}`,
        `active_run=${activeRunIdForSelectedTeam ?? "-"}`,
        `run_status=${activeRunForSelectedTeam?.status ?? "-"}`,
        `context=${activeRunForSelectedTeam?.context_id ?? "-"}`,
        isAgentWorkspace &&
        selectedAgentWorkspaceLiveState?.agent_name &&
        selectedAgentWorkspaceLiveState.agent_name !== selectedAgentWorkspaceLiveState.member_id
          ? `agent=${selectedAgentWorkspaceLiveState.agent_name}`
          : null,
        isAgentWorkspace ? `member=${selectedAgentWorkspaceLiveState?.member_id ?? "-"}` : null,
      ].filter((value): value is string => value !== null),
    [
      activeRunForSelectedTeam?.context_id,
      activeRunForSelectedTeam?.status,
      activeRunIdForSelectedTeam,
      isAgentWorkspace,
      selectedAgentWorkspaceLiveState?.agent_name,
      selectedAgentWorkspaceLiveState?.member_id,
      selectedTeam?.id,
      selectedTeamRuntimeStatus.status,
    ]
  );
  const mailboxDisplayNameByActorId = useMemo(
    () =>
      createDisplayNameLookup([
        [HUMAN_MAILBOX_ACTOR_ID, "You"],
        ...selectedTeamMemberLiveStates.map((member) => [
          member.member_id,
          member.agent_name?.trim() || member.member_id,
        ]),
      ]),
    [selectedTeamMemberLiveStates]
  );
  const onOpenTaskRun = useCallback(
    (runId: string) => {
      setActiveRunId(runId);
      setRunLookupId(runId);
      setFocusedAgentMemberId("");
      setTab("runs");
    },
    [setActiveRunId, setRunLookupId, setTab]
  );
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
      const agentId = selectedAgentWorkspaceAgentId;
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
            if (selectedTeamId) {
              void refreshTeamRuntime(selectedTeamId).catch(() => undefined);
            }
            await loadMemberEvents("replace");
            return;
          } catch (retryErr) {
            setError(parseErrorMessage(retryErr));
            return;
          }
        }
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
      selectedAgentWorkspaceAgentId,
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
  const onForceNewTeamMemberSession = useCallback(async () => {
    if (!props.token || !selectedTeamId || !selectedAgentWorkspaceMemberId) {
      return;
    }
    setError(null);
    setWarning(null);
    setBusy("force-new-session");
    try {
      const runtime = await api.forceTeamMemberNewSession(
        props.token,
        selectedTeamId,
        selectedAgentWorkspaceMemberId
      );
      void Promise.all([
        refreshTeamRuntime(selectedTeamId),
        refreshAgents(),
      ]).catch(() => undefined);
      setWarning(formatTeamRuntimeActionSummary("force", runtime.members));
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentWorkspaceMemberId,
    selectedTeamId,
    setBusy,
    setError,
    setWarning,
  ]);
  const onStartSelectedTeamAgent = useCallback(async () => {
    if (!props.token || !selectedAgentWorkspaceAgent) {
      return;
    }
    setError(null);
    setWarning(null);
    setBusy("start-team-member-agent");
    try {
      await api.startAgent(props.token, selectedAgentWorkspaceAgent.id);
      void Promise.all([
        refreshAgents(),
        selectedTeamId ? refreshTeamRuntime(selectedTeamId) : Promise.resolve(null),
      ]).catch(() => undefined);
      setWarning(`Started ${selectedAgentLabel}.`);
    } catch (err) {
      const message = parseErrorMessage(err);
      if (message.toLowerCase().includes("agent already running")) {
        void Promise.all([
          refreshAgents(),
          selectedTeamId ? refreshTeamRuntime(selectedTeamId) : Promise.resolve(null),
        ]).catch(() => undefined);
        setWarning(`${selectedAgentLabel} is already running.`);
        return;
      }
      setError(message);
    } finally {
      setBusy(null);
    }
  }, [
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentLabel,
    selectedAgentWorkspaceAgent,
    selectedTeamId,
    setBusy,
    setError,
    setWarning,
  ]);
  const onStopSelectedTeamAgent = useCallback(async () => {
    if (!props.token || !selectedAgentWorkspaceAgent) {
      return;
    }
    setError(null);
    setWarning(null);
    setBusy("stop-team-member-agent");
    try {
      await api.stopAgent(props.token, selectedAgentWorkspaceAgent.id);
      void Promise.all([
        refreshAgents(),
        selectedTeamId ? refreshTeamRuntime(selectedTeamId) : Promise.resolve(null),
      ]).catch(() => undefined);
      setWarning(`Stopped ${selectedAgentLabel}.`);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentLabel,
    selectedAgentWorkspaceAgent,
    selectedTeamId,
    setBusy,
    setError,
    setWarning,
  ]);
  const onDeleteSelectedTeamAgent = useCallback(async () => {
    if (!props.token || !selectedAgentWorkspaceAgent || !selectedAgentWorkspaceMemberId) {
      return;
    }
    setError(null);
    setWarning(null);
    setBusy("delete-team-member-agent");
    try {
      await api.deleteAgent(props.token, selectedAgentWorkspaceAgent.id);
      setAgents((prev) =>
        prev.filter((agent) => agent.id !== selectedAgentWorkspaceAgent.id)
      );
      setTeamMemberAgentsById((prev) => ({
        ...prev,
        [selectedAgentWorkspaceMemberId]: null,
      }));
      setMemberDiscoveryCardsById((prev) => {
        return removeTeamMemberLookupEntry(prev, selectedAgentWorkspaceMemberId);
      });
      setMemberDiscoveryCardLoadingById((prev) => {
        return removeTeamMemberLookupEntry(prev, selectedAgentWorkspaceMemberId);
      });
      void Promise.all([
        refreshAgents(),
        selectedTeamId ? refreshTeamRuntime(selectedTeamId) : Promise.resolve(null),
      ]).catch(() => undefined);
      setWarning(
        `Deleted ${selectedAgentLabel}. The Team member remains in the spec until you edit the profile.`
      );
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    selectedAgentLabel,
    selectedAgentWorkspaceAgent,
    selectedAgentWorkspaceMemberId,
    selectedTeamId,
    setBusy,
    setError,
    setWarning,
  ]);
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
  const onStartTeamRuntime = useCallback(async () => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    if (!selectedTeamHasConfiguredMembers) {
      setError(teamExecutionBlockedReason ?? "Add at least one agent first");
      return;
    }
    setBusy("start-team");
    setError(null);
    setWarning(null);
    try {
      const runtime = await api.startTeam(props.token, selectedTeam.id);
      applyOptimisticTeamRuntime(
        selectedTeam.id,
        selectedTeam.name,
        runtime,
        selectedTeamMemberStatuses
      );
      void Promise.all([refreshTeams(), refreshAgents()]).catch((err) => {
        setError(parseErrorMessage(err));
      });
      void refreshTeamRuntime(selectedTeam.id).catch(() => undefined);
      setWarning(formatTeamRuntimeActionSummary("start", runtime.members));
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    applyOptimisticTeamRuntime,
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    refreshTeams,
    selectedTeamHasConfiguredMembers,
    selectedTeamMemberStatuses,
    selectedTeam,
    setBusy,
    setError,
    setWarning,
    teamExecutionBlockedReason,
  ]);
  const onStopTeamRuntime = useCallback(async () => {
    if (!selectedTeam) {
      setError("Select a team first");
      return;
    }
    setBusy("stop-team");
    setError(null);
    setWarning(null);
    try {
      const runtime = await api.stopTeam(props.token, selectedTeam.id);
      applyOptimisticTeamRuntime(
        selectedTeam.id,
        selectedTeam.name,
        runtime,
        selectedTeamMemberStatuses
      );
      void Promise.all([refreshTeams(), refreshAgents()]).catch((err) => {
        setError(parseErrorMessage(err));
      });
      void refreshTeamRuntime(selectedTeam.id).catch(() => undefined);
      setWarning(formatTeamRuntimeActionSummary("stop", runtime.members));
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    applyOptimisticTeamRuntime,
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    refreshTeams,
    selectedTeamMemberStatuses,
    selectedTeam,
    setBusy,
    setError,
    setWarning,
  ]);
  const onRefreshTasks = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    setError(null);
    await refreshTasks(selectedTeamId);
  }, [refreshTasks, selectedTeamId]);
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
  const conversationPanel = (
    <TeamConversationPanel
      developerMode={props.developerMode}
      token={props.token}
      tasksLoading={tasksLoading}
      onRefreshTasks={onRefreshTasks}
      messageDraft={taskMessageDraft}
      onMessageDraftChange={setTaskMessageDraft}
      onSendMessage={onSendTaskMessage}
      onRefreshMessages={refreshTaskMessages}
      messages={taskMessages}
      conversationMailboxMessages={conversationMailboxMessages}
      snapshotMailboxMessages={snapshot?.mailbox.recent_messages ?? []}
      humanActorId={HUMAN_MAILBOX_ACTOR_ID}
      memberLiveStates={selectedTeamMemberLiveStates}
      memberIds={taskConversationMemberIds}
      conversationTitle={activeConversationTitle}
      isSharedConversation={selectedConversationIsShared}
      messagesLoading={taskMessagesLoading}
      busy={busy}
      formatTs={formatTs}
      toPrettyJson={toPrettyJson}
    />
  );

  const tasksPanel = (
    <TeamTasksPanel
      compactMode={isCompactWorkbench}
      developerMode={props.developerMode}
      tasks={workspaceTasks}
      tasksLoading={tasksLoading}
      selectedTaskId={selectedTaskId}
      onSelectedTaskIdChange={setSelectedTaskId}
      onRefreshTasks={onRefreshTasks}
      onOpenConversation={onSelectConversationSubject}
      busy={busy}
      runs={runs}
      onOpenRun={onOpenTaskRun}
      compilePreviewContextId={compilePreviewContextId}
      onCompilePreviewContextIdChange={setCompilePreviewContextId}
      onCompileTaskRunPreview={onCompileTaskRunPreview}
      canCompileTask={canCompileTask}
      compiledRunPreview={compiledRunPreview}
      onUseCompiledRunPayload={onUseCompiledRunPayload}
      onCreateRunFromCompiledPreview={onCreateRunFromCompiledPreview}
      formatTs={formatTs}
      toPrettyJson={toPrettyJson}
      memberLiveStates={selectedTeamMemberLiveStates}
    />
  );

  const teamDebugChrome = useMemo(
    () => ({
      panelCardClassName: TEAM_PANEL_CARD_CLASS,
      sectionHeadingClassName: teamSectionHeadingClassName,
      sectionBodyTextClassName: teamSectionBodyTextClassName,
      sectionHintTextClassName: teamSectionHintTextClassName,
      debugTabsClassName: teamDebugTabsClassName,
      debugTabActiveClassName: teamDebugTabActiveClassName,
      debugTabIdleClassName: teamDebugTabIdleClassName,
      panelSecondaryButtonClassName,
    }),
    []
  );

  const runOpsPanel = (
    <TeamRunOpsPanel
      chrome={teamDebugChrome}
      busy={busy}
      runContextId={runContextId}
      runInput={runInput}
      runLookupId={runLookupId}
      canCreateRun={canCreateRun}
      runInputHasError={runInputHasError}
      runInputError={runInputValidation.error}
      createRunTitle={runInputValidation.error ?? teamExecutionBlockedReason ?? "Create run"}
      parsedRunInput={runInputValidation.parsed}
      helperText={
        teamExecutionBlockedReason ??
        "Accepts any valid JSON value. Shortcut: Ctrl/Cmd + Enter to create run."
      }
      onRunContextIdChange={setRunContextId}
      onRunInputChange={setRunInput}
      onRunLookupIdChange={setRunLookupId}
      onCreateRun={onCreateRun}
      onLoadRunById={onLoadRunById}
      onUseExampleJson={() =>
        setRunInput(
          JSON.stringify(
            {
              task: "investigate",
              objective: "improve-team-run",
            },
            null,
            2
          )
        )
      }
      onSetEmptyObject={() => setRunInput("{}")}
      onFormatJson={() => {
        const parsed = runInputValidation.parsed;
        if (parsed === undefined && runInput.trim().length === 0) {
          setRunInput("{}");
          return;
        }
        if (runInputValidation.error || parsed === undefined) {
          return;
        }
        setRunInput(JSON.stringify(parsed, null, 2));
      }}
      onClearRunInput={() => setRunInput("")}
    />
  );
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
  const teamPanelToggleLabel = isCompactWorkbench
    ? teamsSidebarCollapsed
      ? "Show teams panel"
      : "Show workbench"
    : teamsSidebarCollapsed
      ? "Show teams panel"
      : "Hide teams panel";
  const detailLayoutClassName = isCompactWorkbench
    ? "teams-layout flex min-h-0 flex-1 flex-col"
    : teamsSidebarCollapsed
      ? teamWorkbenchDetailLayoutCollapsedClassName
      : teamWorkbenchDetailLayoutExpandedClassName;

  return (
    <div className={TEAM_PAGE_ROOT_CLASS}>
      <TeamPageHeader
        isSelectorRoute={isSelectorRoute}
        teamsSidebarCollapsed={teamsSidebarCollapsed}
        teamPanelToggleLabel={teamPanelToggleLabel}
        connectionBadge={connectionBadge}
        username={props.auth.username}
        isRoot={props.auth.role === "root"}
        headerShellClassName={teamWorkbenchHeaderShellClassName}
        headerIconButtonClassName={teamWorkbenchHeaderIconButtonClassName}
        headerMutedButtonClassName={teamWorkbenchMutedButtonClassName}
        headerStatusClassName={teamWorkbenchHeaderStatusClassName}
        onToggleSidebar={() => setTeamsSidebarCollapsed((previous) => !previous)}
        onNavigateToSelector={navigateToTeamSelector}
        onNavigate={navigateTeamRoute}
        onLogout={props.onLogout}
      />

      {error && <ErrorBanner message={error} onClose={() => setError(null)} />}
      {warningNotice?.kind === "runtime" && (
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
      )}
      {warningNotice?.kind === "warning" && (
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
      )}

      {isSelectorRoute ? (
        <TeamSelectorPanel
          busy={busy}
          filter={teamSelectorFilter}
          hasTeams={teams.length > 0}
          items={selectorTeamItems}
          bodyTextClassName={teamSectionBodyTextClassName}
          accentButtonClassName={teamWorkbenchAccentButtonClassName}
          onFilterChange={setTeamSelectorFilter}
          onRefreshTeams={() => {
            void refreshTeams();
          }}
          onCreateTeam={openCreateTeamModal}
          onSelectTeam={navigateToTeamDetail}
        />
      ) : (
        <div className={detailLayoutClassName}>
          {showSidebarPane && (
            <TeamSidebar
              showTeamSelector={false}
              developerMode={props.developerMode}
              busy={busy}
              onRefreshTeams={refreshTeams}
              onOpenCreateTeam={openCreateTeamModal}
              draftTeamName={newTeamName}
              leaderMemberId={
                selectedTeamMembers.find((member) => member.role === "leader")?.member_id ?? ""
              }
              configuredWorkerCount={selectedTeamWorkerCount}
              teams={teams}
              selectedTeam={selectedTeam}
              selectedTeamId={effectiveSelectedTeamId}
              selectedTeamRuntimeStatus={selectedTeamRuntimeStatus}
              selectedTeamMemberCount={selectedTeamMembers.length}
              selectedTeamHasConfiguredMembers={selectedTeamHasConfiguredMembers}
              teamMemberSummaryByTeamId={teamMemberSummaryByTeamId}
              memberLiveStates={selectedTeamMemberLiveStates}
              focusedAgentMemberId={focusedAgentMemberId}
              tab={tab}
              onSelectTeam={onSelectSidebarTeam}
              onSelectConversation={onSelectConversationSubject}
              onSelectKanban={onSelectKanbanSubject}
              onSelectAgentTab={onSelectAgentWorkspace}
              onSelectUtilityTab={onSelectUtilityWorkspace}
              onOpenTeamMemberForge={openTeamMemberForgeModal}
              onStartTeamRuntime={onStartTeamRuntime}
              onStopTeamRuntime={onStopTeamRuntime}
            />
          )}

          {showWorkbenchPane && (
          <div
            className="teams-main flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-hidden pb-3 pr-1 lg:mx-auto lg:w-full lg:max-w-[1180px] lg:pr-0"
            data-team-surface="workbench"
          >
            {showTeamBootstrapLoading && <TeamLoadingPanel />}

            {showTeamUnavailable && (
              <TeamUnavailablePanel onBackToSelector={navigateToTeamSelector} />
            )}

            {selectedTeam && (
              <>
                <div
                  className={`${teamSectionCardClassName} ${teamWorkbenchWorkspaceShellClassName} ${
                    isAgentWorkspace ? "py-0.5" : ""
                  }`}
                >
                  <TeamWorkspaceHeader
                    workspaceEyebrow={workspaceEyebrow}
                    showDedicatedWorkspaceHeading={showDedicatedWorkspaceHeading}
                    workspaceTitle={workspaceTitle}
                    workspaceDescription={workspaceDescription}
                    isAgentWorkspace={isAgentWorkspace}
                    selectedAgentLabel={selectedAgentLabel}
                    selectedAgentWorkspaceMemberId={selectedAgentWorkspaceMemberId}
                    selectedAgentStatusView={selectedAgentStatusView}
                    selectedAgentSpecDraft={selectedAgentSpecDraft}
                    selectedAgentControlState={selectedAgentControlState}
                    showWorkspaceRuntimeBadge={showWorkspaceRuntimeBadge}
                    selectedTeamRuntimeStatusLabel={selectedTeamRuntimeStatus.label}
                    selectedTeamRuntimeOnline={selectedTeamRuntimeStatus.online}
                    selectedTeamRuntimeTotal={selectedTeamRuntimeStatus.total}
                    selectedTeamRuntimeControlTone={selectedTeamRuntimeControlTone}
                    workspaceAdvancedTabItems={workspaceAdvancedTabItems}
                    isAdvancedWorkspace={isAdvancedWorkspace}
                    showRunActionsInAdvanced={showRunActionsInAdvanced}
                    activeRunStatus={activeRunForSelectedTeam?.status ?? null}
                    canResumeActiveRun={canResumeActiveRun}
                    canRestartActiveRun={canRestartActiveRun}
                    developerMode={props.developerMode}
                    workspaceDetailsOpen={workspaceDetailsOpen}
                    workspaceDetailItems={workspaceDetailItems}
                    workspaceNoticeText={workspaceNoticeText}
                    workspaceNoticeDotClassName={workspaceNoticeDotClassName}
                    workflowTabItems={TEAM_WORKFLOW_TAB_ITEMS}
                    tab={tab}
                    busy={busy}
                    chrome={{
                      mutedButtonClassName: teamWorkbenchMutedButtonClassName,
                      headerActionButtonClassName: teamWorkbenchHeaderActionButtonClassName,
                      toolbarClassName: workspaceToolbarClassName,
                      toolbarButtonActiveClassName: workspaceToolbarButtonActiveClassName,
                      toolbarButtonIdleClassName: workspaceToolbarButtonIdleClassName,
                      noticeClassName: workspaceNoticeClassName,
                      noticeTextClassName: workspaceNoticeTextClassName,
                      runMetaItemClassName: teamRunMetaItemClassName,
                    }}
                    onTabChange={setTab}
                    onToggleWorkspaceDetails={() =>
                      setWorkspaceDetailsOpen((current) => !current)
                    }
                    onRefreshActiveRun={onRefreshActiveRun}
                    onCancelRun={onCancelRun}
                    onResumeRun={onResumeRun}
                    onRestartRun={onRestartRun}
                    onOpenTeamMemberEditModal={openTeamMemberEditModal}
                    onStartSelectedTeamAgent={onStartSelectedTeamAgent}
                    onStopSelectedTeamAgent={onStopSelectedTeamAgent}
                    onDeleteSelectedTeamAgent={onDeleteSelectedTeamAgent}
                  />
              </div>

              {!selectedTeamHasConfiguredMembers && (
                <TeamSetupPanel
                  description={selectedTeam.description}
                  forgeLabel={teamMemberForgeLabel}
                  onForge={openTeamMemberForgeModal}
                />
              )}

              {tab === "runs" && (
                <TeamRunPanel
                  selectedTeam={selectedTeam}
                  developerMode={props.developerMode}
                  busy={busy}
                  onDeleteTeam={onDeleteTeam}
                  onStartRun={onCreateRun}
                  canStartRun={selectedTeamHasConfiguredMembers}
                  runBlockedReason={teamExecutionBlockedReason}
                  runStatusFilter={runStatusFilter}
                  runStatusFilterOptions={TEAM_RUN_STATUS_FILTER_OPTIONS}
                  onRunStatusFilterChange={onRunStatusFilterChange}
                  onRefreshRuns={onRefreshRuns}
                  runsLoading={runsLoading}
                  visibleRuns={visibleRuns}
                  activeRunId={activeRunIdForSelectedTeam}
                  onActiveRunChange={setActiveRunId}
                  isActiveRunHiddenByFilter={isActiveRunHiddenByFilter}
                  activeRun={activeRunForSelectedTeam}
                  totalLoadedRunsForTeam={totalLoadedRunsForTeam}
                  runsHasMore={runsHasMore}
                  selectedTeamId={effectiveSelectedTeamId}
                  onLoadMoreRuns={onLoadMoreRuns}
                />
              )}

              {showRunContextLoading && (
                <div className={teamSectionCardClassName}>
                  <p className="text-sm text-ui-text-muted">
                    Loading run context for selected team...
                  </p>
                </div>
              )}

              {showNoActiveRunNotice && (
                <div className={teamSectionCardClassName}>
                  <h3 className={teamSectionTitleClassName}>No Active Execution Run</h3>
                  <p className={teamSectionBodyTextClassName}>
                    Select an existing execution run or start one in the Execution Runs tab before opening this panel.
                  </p>
                  <div className="mt-3">
                    <ActionButton
                      tone="secondary"
                      size="md"
                      className={panelSecondaryButtonClassName}
                      onClick={() => setTab("runs")}
                    >
                      Go to Execution Runs
                    </ActionButton>
                  </div>
                </div>
              )}

              {tab !== "runs" && !showRunContextLoading && !showNoActiveRunNotice && (
                <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
                  {tab === "conversation" && (
                    <>
                      {conversationPanel}
                    </>
                  )}

                  {tab === "tasks" && tasksPanel}

                  {tab === "agent_acp" && (
                    <Suspense
                      fallback={
                      <div className={teamSectionCardClassName}>
                        <p className={teamSectionBodyTextClassName}>
                          Loading agent ACP...
                        </p>
                      </div>
                      }
                    >
                      <LazyTeamMemberAcpPanel
                        developerMode={props.developerMode}
                        selectedMemberId={selectedAgentWorkspaceMemberId}
                        memberTitle={selectedAgentLabel}
                        hideMemberTitle={true}
                        selectedMemberSnapshot={selectedAgentWorkspaceSnapshot}
                        selectedMemberRole={
                          selectedAgentWorkspaceRuntimeMember?.role ??
                          selectedAgentWorkspaceSnapshot?.role ??
                          null
                        }
                        selectedSessionId={selectedAgentWorkspaceSessionId}
                        memberEvents={memberEvents}
                        memberEventsHasMore={memberEventsHasMore}
                        memberEventsLoading={memberEventsLoading}
                        eventsLoading={eventsLoading}
                        oldestMemberEventId={oldestMemberEventId}
                        onSendInput={onSendAgentAcpInput}
                        canControlAcp={isAgentActiveStatus(
                          selectedAgentWorkspaceAgent?.status ?? null
                        )}
                        canInterrupt={isAgentActiveStatus(
                          selectedAgentWorkspaceAgent?.status ?? null
                        )}
                        onInterrupt={onCancelTeamMemberAcp}
                        onAcpSetMode={onSetTeamMemberAcpMode}
                        onAcpSetModel={onSetTeamMemberAcpModel}
                        onAcpSetConfig={onSetTeamMemberAcpConfig}
                        onForceNewSession={onForceNewTeamMemberSession}
                        onLoadOlder={onLoadOlderMemberConsole}
                      />
                    </Suspense>
                  )}

                  {tab === "overview" && activeRunForSelectedTeam && (
                  <TeamOverviewPanel
                    snapshot={snapshot}
                    snapshotLoading={snapshotLoading}
                    onRefreshSnapshot={onRefreshOverviewSnapshot}
                    selectedMemberId={selectedMemberId}
                    onOpenMailboxForMember={onOpenMailboxForMember}
                    displayNameByActorId={mailboxDisplayNameByActorId}
                  />
                  )}

                  {tab === "events" && activeRunForSelectedTeam && (
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

                  {tab === "steps" && activeRunForSelectedTeam && (
                    <TeamStepsPanel
                      developerMode={props.developerMode}
                      mode="list_only"
                      steps={steps}
                      onRefreshSteps={onRefreshActiveRunSteps}
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

                  {tab === "mailbox" && !activeRunForSelectedTeam && (
                    <div className={teamSectionCardClassName}>
                      <h3 className={teamSectionTitleClassName}>
                        {isAgentWorkspace ? selectedAgentLabel : "Execution Mailbox"}
                      </h3>
                      <p className={teamSectionBodyTextClassName}>
                        {isAgentWorkspace
                          ? "This agent is selected, but there is no active execution run context for its direct thread yet. Use Execution Runs to inspect execution history or wait for the next task."
                          : "Execution mailbox is run-scoped. Start or select a run to inspect delivery and direct member conversations."}
                      </p>
                      <div className="mt-3">
                        <ActionButton
                          tone="secondary"
                          size="md"
                          className={panelSecondaryButtonClassName}
                          onClick={() => setTab("runs")}
                        >
                          Go to Execution Runs
                        </ActionButton>
                      </div>
                    </div>
                  )}

                  {tab === "mailbox" && activeRunForSelectedTeam && (
                    <TeamMailboxPanel
                      developerMode={props.developerMode}
                      mode="full"
                      snapshot={snapshot}
                      humanActorId={HUMAN_MAILBOX_ACTOR_ID}
                      selectedMemberId={selectedMemberId}
                      unreadByMemberId={unreadByMemberId}
                      onSelectMember={setSelectedMemberId}
                      chatActors={chatActors}
                      chatStickToBottom={chatStickToBottom}
                      chatMessagesRef={chatMessagesRef}
                      onConversationScroll={onConversationScroll}
                      onJumpToBottom={onJumpConversationToBottom}
                      conversationMessages={conversationMessages}
                      displayNameByActorId={mailboxDisplayNameByActorId}
                      toPrettyJson={toPrettyJson}
                      formatTs={formatTs}
                      busy={busy}
                      onAcceptMessage={onAcceptMessage}
                      onAcceptVisibleMessages={onAcceptVisibleMessages}
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
                      onMsgTemplateChange={onMailboxTemplateChange}
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

                  {tab === "member_console" && activeRunForSelectedTeam && (
                  <TeamMemberConsolePanel
                    snapshot={snapshot}
                    selectedMemberId={selectedMemberId}
                    onSelectedMemberIdChange={setSelectedMemberId}
                    selectedMemberSnapshot={selectedMemberSnapshot}
                    displayNameByActorId={mailboxDisplayNameByActorId}
                    memberEvents={memberEvents}
                      memberEventsHasMore={memberEventsHasMore}
                      memberEventsLoading={memberEventsLoading}
                      eventsLoading={eventsLoading}
                      oldestMemberEventId={oldestMemberEventId}
                      displayedRunEvents={displayedRunEvents}
                      previewLimit={TEAM_EVENT_PREVIEW_LIMIT}
                      memberDiscoveryCard={selectedMemberDiscoveryCard}
                      memberDiscoveryCardLoading={selectedMemberDiscoveryCardLoading}
                      onRefresh={onRefreshMemberConsole}
                      onLoadOlder={onLoadOlderMemberConsole}
                      toPrettyJson={toPrettyJson}
                      formatTs={formatTs}
                    />
                  )}

                  {tab === "debug" && props.developerMode && (
                    <>
                      <TeamDebugToolsHeader
                        chrome={teamDebugChrome}
                        teamDebugTag={teamDebugTag}
                        onTeamDebugTagChange={setTeamDebugTag}
                      />

                      {teamDebugTag === "run_ops" && runOpsPanel}

                      {teamDebugTag === "step_ops" && !activeRunForSelectedTeam && (
                        <TeamRunRequiredPanel
                          chrome={teamDebugChrome}
                          title="Step Ops"
                          body="Step operations require an active execution run. Start or select one in the Execution Runs tab first."
                          onGoToRuns={() => setTab("runs")}
                        />
                      )}

                      {teamDebugTag === "step_ops" && activeRunForSelectedTeam && (
                        <TeamStepsPanel
                          developerMode={props.developerMode}
                          mode="controls_only"
                          steps={steps}
                          onRefreshSteps={onRefreshActiveRunSteps}
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

                      {teamDebugTag === "mailbox_raw" && !activeRunForSelectedTeam && (
                        <TeamRunRequiredPanel
                          chrome={teamDebugChrome}
                          title="Mailbox Raw"
                          body="Mailbox raw operations require an active execution run. Start or select one in the Execution Runs tab first."
                          onGoToRuns={() => setTab("runs")}
                        />
                      )}

                      {teamDebugTag === "mailbox_raw" && activeRunForSelectedTeam && (
                        <TeamMailboxPanel
                          developerMode={props.developerMode}
                          mode="advanced_only"
                          snapshot={snapshot}
                          humanActorId={HUMAN_MAILBOX_ACTOR_ID}
                          selectedMemberId={selectedMemberId}
                          unreadByMemberId={unreadByMemberId}
                          onSelectMember={setSelectedMemberId}
                          chatActors={chatActors}
                          chatStickToBottom={chatStickToBottom}
                          chatMessagesRef={chatMessagesRef}
                          onConversationScroll={onConversationScroll}
                          onJumpToBottom={onJumpConversationToBottom}
                          conversationMessages={conversationMessages}
                          displayNameByActorId={mailboxDisplayNameByActorId}
                          toPrettyJson={toPrettyJson}
                          formatTs={formatTs}
                          busy={busy}
                          onAcceptMessage={onAcceptMessage}
                          onAcceptVisibleMessages={onAcceptVisibleMessages}
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
                          onMsgTemplateChange={onMailboxTemplateChange}
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
                    </>
                  )}
                </div>
              )}
            </>
          )}
          </div>
          )}
        </div>
      )}

      <TeamCreateDialog
        open={showCreateTeamModal}
        busy={busy}
        teamName={newTeamName}
        teamDescription={newTeamDescription}
        onTeamNameChange={setNewTeamName}
        onTeamDescriptionChange={setNewTeamDescription}
        onCreateTeam={onCreateTeam}
        onClose={closeCreateTeamModal}
        chrome={{
          panelClassName: teamWorkbenchPanelClassName,
          accentButtonClassName: teamWorkbenchAccentButtonClassName,
          mutedButtonClassName: teamWorkbenchMutedButtonClassName,
          badgeClassName: teamWorkbenchBadgeClassName,
          modalHeaderClassName:
            "modal-head flex flex-wrap items-start justify-between gap-3 border-b border-notion-border pb-4",
          infoStripItemClassName: teamWorkbenchInfoStripItemClassName,
          infoStripLabelClassName: teamWorkbenchInfoStripLabelClassName,
          infoStripValueClassName: teamWorkbenchInfoStripValueClassName,
        }}
      />

      <TeamForgeAgentDialog
        open={showForgeAgentForm}
        draft={teamMemberDraft}
        roleProfile={teamMemberRoleProfile}
        roleOptions={teamMemberRoleOptions}
        selectedTeamHasLeader={selectedTeamHasLeader}
        onRoleChange={handleTeamMemberRoleChange}
        onPatchDraft={patchTeamMemberDraft}
        chrome={{
          panelClassName: teamWorkbenchPanelClassName,
          accentButtonClassName: teamWorkbenchAccentButtonClassName,
          mutedButtonClassName: teamWorkbenchMutedButtonClassName,
          badgeClassName: teamWorkbenchBadgeClassName,
          modalHeaderClassName:
            "modal-head flex flex-wrap items-start justify-between gap-3 border-b border-notion-border pb-4",
          setupChecklistClassName: teamWorkbenchSetupChecklistClassName,
          infoStripGridClassName: teamWorkbenchInfoStripGridClassName,
          infoStripItemClassName: teamWorkbenchInfoStripItemClassName,
          infoStripLabelClassName: teamWorkbenchInfoStripLabelClassName,
          infoStripValueClassName: teamWorkbenchInfoStripValueClassName,
        }}
        modalProps={{
          title: "Add Agent",
          confirmLabel: "Create Agent",
          agentPresetLabel: "Role model",
          agentPresetSummaryLabel: "Model",
          teamStyled: true,
          agentName: forgeAgentName,
          setAgentName: setForgeAgentName,
          agentWorkdir: forgeAgentWorkdir,
          setAgentWorkdir: setForgeAgentWorkdir,
          agentPresetId: forgeAgentPresetId,
          setAgentPresetId: setForgeAgentPresetId,
          worktreeMode: forgeAgentWorktreeMode,
          setWorktreeMode: handleForgeWorktreeModeChange,
          worktreeRepo: forgeAgentWorktreeRepo,
          setWorktreeRepo: setForgeAgentWorktreeRepo,
          worktreeRef: forgeAgentWorktreeRef,
          setWorktreeRef: setForgeAgentWorktreeRef,
          codeMode: forgeAgentCodeMode,
          setCodeMode: setForgeAgentCodeMode,
          worktreeError: forgeAgentWorktreeError,
          showWorktreeAdvancedOptions: teamMemberDraft?.role !== "leader",
          createBusy: forgeAgentBusy,
          workdirPlaceholder: forgeDefaultWorktreeRoot,
          withinPortal: true,
          onCreateAgent: onCreateForgeAgent,
          onClose: closeTeamMemberForgeModal,
        }}
      />

      <TeamEditMemberDialog
        open={showTeamMemberEditModal}
        busy={busy}
        selectedAgentLabel={selectedAgentLabel}
        draft={teamMemberEditDraft}
        onPatchDraft={patchTeamMemberEditDraft}
        onClose={closeTeamMemberEditModal}
        onSave={onSaveTeamMemberProfile}
        chrome={{
          panelClassName: teamWorkbenchPanelClassName,
          accentButtonClassName: teamWorkbenchAccentButtonClassName,
          mutedButtonClassName: teamWorkbenchMutedButtonClassName,
          badgeClassName: teamWorkbenchBadgeClassName,
          modalHeaderClassName:
            "modal-head flex flex-wrap items-start justify-between gap-3 border-b border-notion-border pb-4",
          infoStripItemClassName: teamWorkbenchInfoStripItemClassName,
          infoStripLabelClassName: teamWorkbenchInfoStripLabelClassName,
          infoStripValueClassName: teamWorkbenchInfoStripValueClassName,
        }}
      />
    </div>
  );
}
