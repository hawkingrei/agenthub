import React, { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  Alert,
  Badge,
  Button,
  Group,
  Menu,
  SegmentedControl,
  Tabs,
  TextInput,
  Textarea,
  Tooltip,
} from "@mantine/core";
import { deriveConnectionBadge, getNavigatorOnline } from "../connection_status";
import {
  AGENT_SOURCE_TEAM_FORGE,
  AgentDiscoveryCardRecord,
  AgentRecord,
  AgentEvent,
  api,
  getTeamStepRuntimeHandleId,
  TeamConversationMessageRecord,
  TeamActorMessageRecord,
  TeamDefinitionRecord,
  TeamRuntimeRecord,
  TeamTaskRecord,
  TeamTaskRunCompilePreviewRecord,
  TeamTaskStatus,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamRunSnapshotRecord,
  TeamStepRecord,
} from "../api";
import { AGENT_NOT_RUNNING_ERROR } from "../agent_ws";
import {
  DEFAULT_AGENT_PRESET_ID,
  getAgentPreset,
  type AgentPresetId,
} from "../agent_presets";
import { CreateAgentModal } from "../components/create_agent_modal";
import { WorkbenchConnectionBadge } from "../components/workbench_connection_badge";
import { WorkbenchHeaderMenu } from "../components/workbench_header_menu";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";
import {
  normalizeWorkdirInput,
  resolveWorkdirForModeChange,
} from "../worktree_defaults";
import { TeamEventsPanel } from "./team_events_panel";
import { TeamMailboxPanel } from "./team_mailbox_panel";
import { TeamMemberAcpPanel } from "./team_member_acp_panel";
import { TeamMemberConsolePanel } from "./team_member_console_panel";
import { normalizeTeamMemberLifecycle } from "./team_member_status_strip";
import { TeamTaskPanel } from "./team_task_panel";
import { TeamTasksPanel } from "./team_tasks_panel";
import { TeamOverviewPanel } from "./team_overview_panel";
import { TeamRunPanel } from "./team_run_panel";
import { TeamSidebar } from "./team_sidebar";
import { TeamStepsPanel } from "./team_steps_panel";
import {
  appendTeamMemberToSpec,
  buildEmptyTeamSpec,
  buildLeaderForgeDefaultWorkdir,
  formatTeamForgeWorktreeError,
  parseErrorMessage,
  teamSpecHasConfiguredMembers,
  teamSpecHasLeader,
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
  buildMailboxChatPayload,
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
  REQUIRED_TEAM_LEADER_SKILLS,
  REQUIRED_TEAM_WORKER_SKILLS,
  TEAM_SKILL_OPTIONS,
  TeamMemberAgentStatus,
  TeamMemberAgentStatusSummary,
  buildTeamMemberLiveStates,
  parseTeamSpecMembers,
  resolveTeamMemberAgentStatuses,
  summarizeTeamMemberAgentStatuses,
  toggleSkillSelection,
} from "./team/member_helpers";
import {
  DEFAULT_TEAM_THREAD_TITLE,
  DEFAULT_TEAM_THREAD_BOOTSTRAP_KIND,
  formatTs,
  listTeamWorkspaceTasks,
  refreshTeamConversationMailboxAfterSend,
  resolveAgentWorkspaceStatusView,
  resolveTeamPageNotice,
  resolveSelectedAgentWorkspaceLabel,
  isSharedThreadTask,
  resolveSelectedTeamTask,
  resolveTaskConversationMemberIds,
  resolveTaskMessageSeenByActors,
  resolveTeamConversationTask,
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
import { useTeamConversationEffects } from "./team/use_team_conversation_effects";
import { useTeamMemberAgentBackfillEffect } from "./team/use_team_member_agent_backfill_effect";
import { useTeamMailboxLifecycleEffects } from "./team/use_team_mailbox_lifecycle_effects";
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
  TEAM_RUN_PAGE_LIMIT,
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
  TEAM_CREATE_ACTIONS_BAR_CLASS,
  TEAM_CREATE_MODAL_BACKDROP_CLASS,
  TEAM_CREATE_MODAL_CARD_CLASS,
  TEAM_CREATE_PANEL_CARD_CLASS,
  TEAM_CREATE_SKILL_TAG_IDLE_CLASS,
  TEAM_CREATE_SKILL_TAG_SELECTED_CLASS,
  TEAM_PANEL_CARD_CLASS,
} from "../ui/tailwind_classes";

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
  DEFAULT_TEAM_LEADER_PROMPT,
  DEFAULT_TEAM_LEADER_SKILLS,
  DEFAULT_TEAM_WORKER_PROMPT,
  DEFAULT_TEAM_WORKER_SKILLS,
  REQUIRED_TEAM_LEADER_SKILLS,
  REQUIRED_TEAM_WORKER_SKILLS,
  TEAM_SKILL_OPTIONS,
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
};
type TeamDebugTag = "run_ops" | "step_ops" | "mailbox_raw";
type TeamCreateNoteTone = "info" | "warning";

function parseTeamAgentInputSessionMismatch(
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

function buildTeamDetailPath(teamId: string): string {
  return `/teams/${encodeURIComponent(teamId)}`;
}

function navigateTeamRoute(pathname: string): void {
  if (location.pathname === pathname) {
    return;
  }
  window.history.pushState({}, "", pathname);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

const TEAM_PRIMARY_WORKSPACE_TABS = new Set<TeamTab>([
  "conversation",
  "tasks",
]);
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

function formatTeamRuntimeActionSummary(
  action: "start" | "stop",
  members: ReadonlyArray<{ action: string }>
): string {
  const counts = members.reduce<Record<string, number>>((acc, member) => {
    acc[member.action] = (acc[member.action] ?? 0) + 1;
    return acc;
  }, {});
  const parts = Object.entries(counts)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${value}`);
  const prefix = action === "start" ? "Team runtime updated" : "Team runtime stopped";
  return parts.length > 0 ? `${prefix} (${parts.join(", ")})` : prefix;
}

function validateRunInputJson(raw: string): RunInputValidation {
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

const panelSecondaryButtonClassName =
  "inline-flex items-center justify-center rounded-[12px] border border-ui-border bg-white px-2.5 py-1.5 text-[13px] font-semibold text-ui-text-primary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft disabled:cursor-not-allowed disabled:opacity-60";
const teamSectionCardClassName =
  "min-h-0 min-w-0 rounded-[18px] border border-ui-border bg-[linear-gradient(180deg,rgba(252,251,247,0.98)_0%,rgba(244,241,233,0.98)_100%)] p-3 shadow-sm";
const teamSectionCardLargeClassName =
  "min-h-0 rounded-[20px] border border-ui-border bg-[linear-gradient(180deg,rgba(252,251,247,0.98)_0%,rgba(244,241,233,0.98)_100%)] p-3.5 shadow-sm";
const teamSectionHeadingClassName =
  "text-[10px] font-semibold uppercase tracking-[0.16em] text-ui-text-muted";
const teamSectionTitleClassName = "text-base font-semibold tracking-tight text-black";
const teamSectionBodyTextClassName = "mt-2 text-[13px] leading-5 text-ui-text-secondary";
const teamSectionHintTextClassName = "mt-2 text-[12px] leading-5 text-ui-text-muted";
const teamDebugTabsClassName =
  "flex flex-wrap items-center gap-2 border-b border-ui-border pb-1";
const teamDebugTabBaseClassName =
  "rounded-none border-b-2 border-transparent px-0.5 py-1 text-[10px] font-semibold uppercase tracking-[0.1em] transition";
const teamDebugTabActiveClassName =
  `${teamDebugTabBaseClassName} border-brand-primary bg-transparent text-ui-text-primary`;
const teamDebugTabIdleClassName =
  `${teamDebugTabBaseClassName} bg-transparent text-ui-text-muted hover:border-ui-border hover:text-ui-text-primary`;
const teamCreateModalHeaderClassName =
  "modal-head flex flex-wrap items-start justify-between gap-3 border-b border-ui-border pb-4";
const teamRunMetaItemClassName =
  "rounded-[12px] border border-ui-border bg-ui-surface px-2.5 py-1.5 text-[11px] text-ui-text-primary shadow-sm";
const workspaceToolbarClassName =
  "flex flex-wrap items-center gap-2";
const workspacePrimaryTabsListClassName =
  "flex flex-wrap items-center gap-3 border-b border-ui-border pb-1";
const workspaceToolbarButtonBaseClassName =
  "inline-flex items-center gap-1 rounded-none border-b-2 border-transparent px-0.5 py-1 text-[10px] font-semibold uppercase tracking-[0.1em] transition";
const workspaceToolbarButtonActiveClassName =
  `${workspaceToolbarButtonBaseClassName} border-brand-primary bg-transparent text-ui-text-primary`;
const workspaceToolbarButtonIdleClassName =
  `${workspaceToolbarButtonBaseClassName} bg-transparent text-ui-text-muted hover:border-ui-border hover:text-ui-text-primary`;
const workspacePrimaryTabClassName =
  "rounded-none border-b-2 border-transparent px-0.5 py-1 text-[10px] font-semibold uppercase tracking-[0.1em] text-ui-text-muted transition hover:border-ui-border hover:text-ui-text-primary data-[active=true]:border-brand-primary data-[active=true]:bg-transparent data-[active=true]:text-ui-text-primary";
const workspaceNoticeClassName =
  "mt-3 flex flex-wrap items-center justify-between gap-2 rounded-[14px] border border-ui-border bg-ui-surface px-3 py-2.5 shadow-sm";
const workspaceNoticeTextClassName =
  "flex min-w-0 flex-1 items-center gap-2 text-[11px] font-medium uppercase tracking-[0.1em] text-ui-text-muted";
const workspaceNoticeDotBaseClassName =
  "inline-flex h-2.5 w-2.5 shrink-0 rounded-full";
const teamRuntimeNoticeClassName =
  "mb-4 flex items-start justify-between gap-3 rounded-[18px] border border-emerald-200 bg-emerald-50/90 px-4 py-3 text-emerald-950 shadow-sm";
const teamRuntimeNoticeTitleClassName =
  "text-[11px] font-semibold uppercase tracking-[0.14em] text-emerald-800";
const teamRuntimeNoticeBodyClassName = "mt-1 text-sm leading-5 text-emerald-900";
const workspaceMetaDropdownClassName =
  "absolute right-0 top-full z-20 mt-2 flex min-w-64 flex-col gap-2 rounded-[14px] border border-ui-border bg-ui-surface p-2.5 shadow-lg";
const TEAM_PRIMARY_WORKSPACE_ITEMS: ReadonlyArray<{
  value: TeamTab;
  label: string;
  icon: string;
}> = [
  { value: "conversation", label: "all", icon: "bi bi-chat-square-text" },
  { value: "tasks", label: "Kanban", icon: "bi bi-list-check" },
];

const TEAM_CREATE_NOTE_ALERT_CONFIG: Record<
  TeamCreateNoteTone,
  { color: "blue" | "yellow"; title: string; iconClassName: string }
> = {
  info: {
    color: "blue",
    title: "Team note",
    iconClassName: "bi bi-info-circle",
  },
  warning: {
    color: "yellow",
    title: "Action required",
    iconClassName: "bi bi-exclamation-triangle",
  },
};

const TeamCreateNote = React.memo(function TeamCreateNote({
  tone,
  children,
  action,
}: {
  tone: TeamCreateNoteTone;
  children: React.ReactNode;
  action?: React.ReactNode;
}) {
  const config = TEAM_CREATE_NOTE_ALERT_CONFIG[tone];
  return (
    <Alert
      color={config.color}
      variant="light"
      radius="xl"
      mt="md"
      title={config.title}
      icon={<i className={config.iconClassName} aria-hidden="true" />}
    >
      <div className="text-sm text-ui-text-secondary">{children}</div>
      {action ? <div className="mt-3">{action}</div> : null}
    </Alert>
  );
});

const teamWorkbenchPanelClassName =
  "rounded-[18px] border border-ui-border bg-[linear-gradient(180deg,rgba(252,251,247,0.96)_0%,rgba(244,241,233,0.96)_100%)] p-3.5 shadow-sm";
const teamWorkbenchAccentButtonClassName =
  "!border !border-ui-border-emphasis !bg-[#203b2d] !text-white !shadow-sm transition hover:!border-ui-border-strong hover:!bg-[#1b3126]";
const teamWorkbenchMutedButtonClassName =
  "!border !border-ui-border !bg-white !text-ui-text-primary !shadow-sm transition hover:!border-ui-border-emphasis hover:!bg-ui-surface-soft";
const teamWorkbenchHeaderActionsClassName = "flex flex-wrap items-center gap-2";
const teamWorkbenchHeaderActionButtonClassName = "!shrink-0 !whitespace-nowrap";
const teamWorkbenchBadgeClassName =
  "inline-flex items-center rounded-full border border-ui-border bg-ui-surface-soft px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.14em] text-ui-text-muted";
const teamWorkbenchHeaderShellClassName =
  "flex flex-wrap items-center justify-between gap-3 rounded-[18px] border border-ui-border bg-[linear-gradient(180deg,rgba(252,251,247,0.98)_0%,rgba(244,241,233,0.98)_100%)] px-3.5 py-3 shadow-sm";
const teamWorkbenchHeaderIconButtonClassName =
  "inline-flex h-10 w-10 items-center justify-center rounded-[12px] border border-ui-border bg-white text-ui-text-primary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft";
const teamWorkbenchHeaderStatusClassName =
  "inline-flex items-center gap-2 rounded-full border border-ui-border bg-ui-surface px-3 py-1.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted shadow-sm";
const teamWorkbenchWorkspaceShellClassName =
  "rounded-[20px] border border-ui-border bg-[linear-gradient(180deg,rgba(252,251,247,0.98)_0%,rgba(244,241,233,0.98)_100%)] px-3.5 py-3.5 shadow-sm";
const teamWorkbenchSetupChecklistClassName =
  "overflow-hidden rounded-[16px] border border-ui-border bg-ui-surface shadow-sm";
const teamWorkbenchInfoStripGridClassName =
  "grid gap-px bg-ui-border lg:grid-cols-3";
const teamWorkbenchInfoStripItemClassName =
  "min-w-0 bg-ui-surface px-3.5 py-3";
const teamWorkbenchInfoStripLabelClassName =
  "text-[10px] font-semibold uppercase tracking-[0.14em] text-ui-text-muted";
const teamWorkbenchInfoStripValueClassName =
  "mt-1.5 text-[13px] leading-5 text-ui-text-primary";

export function TeamPage(props: TeamPageProps) {
  const routeTeamId = props.routeTeamId?.trim() || null;
  const isSelectorRoute = routeTeamId == null;
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
  const connectionBadge = useMemo(
    () => deriveConnectionBadge(networkOnline, false, "idle"),
    [networkOnline]
  );
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
  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(routeTeamId);
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
  const [teamMemberDraft, setTeamMemberDraft] = useState<TeamMemberProfileDraft | null>(null);
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
  const [tasksLoading, setTasksLoading] = useState(false);
  const [selectedTaskId, setSelectedTaskId] = useState("");
  const [taskMessages, setTaskMessages] = useState<TeamConversationMessageRecord[]>([]);
  const [conversationMailboxMessages, setConversationMailboxMessages] = useState<
    TeamActorMessageRecord[]
  >([]);
  const [taskMessagesLoading, setTaskMessagesLoading] = useState(false);
  const [taskMessageDraft, setTaskMessageDraft] = useState("");
  const [newTaskTitle, setNewTaskTitle] = useState("");
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

  const selectedTeam = useMemo(
    () => teams.find((team) => team.id === selectedTeamId) ?? null,
    [teams, selectedTeamId]
  );
  useEffect(() => {
    setSelectedTeamId(routeTeamId);
  }, [routeTeamId]);
  useEffect(() => {
    setCompiledRunPreview(null);
    setCompilePreviewContextId("");
    setTaskList([]);
    setTasksLoading(false);
    setSelectedTaskId("");
    setTaskMessages([]);
    setTaskMessagesLoading(false);
    setTaskMessageDraft("");
    setNewTaskTitle("");
    setSelectedMemberId("");
    setFocusedAgentMemberId("");
  }, [selectedTeamId, setSelectedMemberId]);
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
  const normalizedTeamSelectorFilter = teamSelectorFilter.trim().toLowerCase();
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
  const primaryWorkspaceTabValue = TEAM_PRIMARY_WORKSPACE_TABS.has(tab) ? tab : null;
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
    if (!activeRun || !selectedTeamId) {
      return null;
    }
    if (activeRun.team_id !== selectedTeamId) {
      return null;
    }
    return activeRun;
  }, [activeRun, selectedTeamId]);
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
    if (!activeRunForSelectedTeam || !selectedTeamId) return false;
    if (runStatusFilter === "all") return false;
    return activeRunForSelectedTeam.status !== runStatusFilter;
  }, [activeRunForSelectedTeam, runStatusFilter, selectedTeamId]);

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
  }, [patchTeamCreate]);

  useEffect(() => {
    if (!props.token) {
      setForgeDefaultWorktreeRoot(DEFAULT_WORKTREE_ROOT);
      return;
    }
    let active = true;
    void api
      .getRuntimeDefaults(props.token)
      .then((defaults) => {
        if (!active) {
          return;
        }
        const root = normalizeWorkdirInput(defaults.default_worktree_root);
        setForgeDefaultWorktreeRoot(root || DEFAULT_WORKTREE_ROOT);
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
    selectedTeamId,
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
    selectedMemberId: selectedAgentWorkspaceMemberId,
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

  const { onSendChatMessage, onSendMessage, onRefreshInbox, onAckMessage } =
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
    selectedTeamId,
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
        created
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
      navigateToTeamSelector();
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

  const onApplyMessageTemplate = () => {
    setMsgPayload(toPrettyJson(buildMailboxPayloadTemplate(msgTemplate)));
  };

  const selectedConversation = useMemo(() => {
    if (!selectedTeamId) {
      return null;
    }
    return resolveTeamConversationTask(taskList, selectedTeamId);
  }, [selectedTeamId, taskList]);
  const workspaceTasks = useMemo(() => {
    if (!selectedTeamId) {
      return [];
    }
    return listTeamWorkspaceTasks(taskList, selectedTeamId);
  }, [selectedTeamId, taskList]);

  const selectedTask = useMemo(() => {
    if (!selectedTeamId) {
      return null;
    }
    return resolveSelectedTeamTask(taskList, selectedTaskId, selectedTeamId);
  }, [selectedTaskId, selectedTeamId, taskList]);
  const conversationSeenByMessageId = useMemo(
    () =>
      resolveTaskMessageSeenByActors(
        mergeMailboxMessages(
          snapshot?.mailbox.recent_messages ?? [],
          conversationMailboxMessages
        ),
        taskMessages[0]?.conversation_id ?? "",
        taskConversationMemberIds
      ),
    [conversationMailboxMessages, snapshot?.mailbox.recent_messages, taskConversationMemberIds, taskMessages]
  );

  const refreshTasks = useCallback(
    async (teamId: string) => {
      setTasksLoading(true);
      try {
        const list = await api.listTeamTasks(props.token, teamId, 100);
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

  useEffect(() => {
    setCompiledRunPreview(null);
    setCompilePreviewContextId("");
  }, [selectedTaskId, selectedTeamId]);

  const refreshTaskMessages = useCallback(
    async (taskIdOverride?: string) => {
      const teamId = selectedTeamId;
      const taskId = (taskIdOverride ?? selectedConversation?.id ?? "").trim();
      if (!teamId || !taskId) {
        setTaskMessages([]);
        setConversationMailboxMessages([]);
        return;
      }
      setTaskMessagesLoading(true);
      try {
        const taskRecord =
          (selectedConversation?.id ?? "").trim() === taskId
            ? selectedConversation
            : taskList.find((task) => task.id === taskId) ?? null;
        const [messages, taskDetail] = await Promise.all([
          api.listTeamTaskMessages(props.token, teamId, taskId, {
            limit: 200,
          }),
          taskRecord && isSharedThreadTask(taskRecord)
            ? api.getTeamTask(props.token, teamId, taskId)
            : Promise.resolve(null),
        ]);
        setTaskMessages(messages);
        const conversationRunId = taskDetail?.latest_run?.id?.trim() ?? "";
        if (conversationRunId) {
          const conversationSnapshot = await api.getTeamRunSnapshot(props.token, conversationRunId, {
            event_limit: 1,
            message_limit: 200,
          });
          setConversationMailboxMessages(conversationSnapshot.mailbox.recent_messages);
        } else {
          setConversationMailboxMessages([]);
        }
      } catch (err) {
        setError(parseErrorMessage(err));
        setConversationMailboxMessages([]);
      } finally {
        setTaskMessagesLoading(false);
      }
    },
    [props.token, selectedConversation, selectedTeamId, setError, taskList]
  );

  useEffect(() => {
    if (!selectedTeamId) {
      return;
    }
    void refreshTasks(selectedTeamId);
  }, [refreshTasks, selectedTeamId]);

  useTeamConversationEffects({
    token: props.token,
    selectedTeamId,
    selectedConversationId: selectedConversation?.id ?? null,
    tab,
    eventsAutoRefresh,
    refreshTaskMessages,
    setTaskMessages,
    setConversationMailboxMessages,
  });

  const resolveConversationForMessage = useCallback(() => {
    if (!selectedTeamId || !selectedConversation) {
      return null;
    }
    return selectedConversation;
  }, [selectedConversation, selectedTeamId]);

  const ensureSharedConversation = useCallback(async () => {
    if (!selectedTeamId) {
      return null;
    }
    const existing = resolveConversationForMessage();
    if (existing) {
      return existing;
    }
    const remoteTasks = sortTasksByActivity(await api.listTeamTasks(props.token, selectedTeamId, 100));
    const remoteConversation = resolveTeamConversationTask(remoteTasks, selectedTeamId);
    if (remoteConversation) {
      setTaskList(remoteTasks);
      return remoteConversation;
    }

    const created = await api.createTeamTask(props.token, selectedTeamId, {
      title: DEFAULT_TEAM_THREAD_TITLE,
      conversation_mode: "group_chat",
      topic: DEFAULT_TEAM_THREAD_TITLE,
      context: {
        bootstrap_kind: DEFAULT_TEAM_THREAD_BOOTSTRAP_KIND,
        bootstrap_source: "teams_all",
      },
    });
    setTaskList((prev) => sortTasksByActivity([created.task, ...prev.filter((task) => task.id !== created.task.id)]));
    setTaskMessages([]);
    return created.task;
  }, [props.token, resolveConversationForMessage, selectedTeamId]);

  const onCreateTask = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    const title = newTaskTitle.trim();
    if (!title) {
      setError("Task title is required");
      return;
    }
    setBusy("create-task");
    setError(null);
    try {
      const created = await api.createTeamTask(props.token, selectedTeamId, {
        title,
        conversation_mode: "to_leader",
        topic: title,
        context: {
          bootstrap_kind: "task_workspace",
        },
      });
      setTaskList((prev) =>
        sortTasksByActivity([created.task, ...prev.filter((task) => task.id !== created.task.id)])
      );
      setSelectedTaskId(created.task.id);
      setCompiledRunPreview(null);
      setNewTaskTitle("");
      if (created.latest_run) {
        applyCreatedRunState(created.latest_run, false);
      }
      setTab("tasks");
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [applyCreatedRunState, newTaskTitle, props.token, selectedTeamId, setBusy, setTab]);

  const onUpdateTaskStatus = useCallback(
    async (taskId: string, status: TeamTaskStatus) => {
      if (!selectedTeamId) {
        setError("Select a team first");
        return;
      }
      setBusy("update-task-status");
      setError(null);
      try {
        const updated = await api.updateTeamTask(props.token, selectedTeamId, taskId, { status });
        setTaskList((prev) =>
          sortTasksByActivity([updated, ...prev.filter((task) => task.id !== updated.id)])
        );
        setSelectedTaskId(updated.id);
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
        setBusy(null);
      }
    },
    [props.token, selectedTeamId]
  );

  const onSendTaskMessage = useCallback(async (payload: {
    text: string;
    mentionActorIds: string[];
  }) => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    const text = payload.text.trim();
    if (!text) {
      setError("Conversation message is required");
      return;
    }
    setBusy("send-task-message");
    setError(null);
    setWarning(null);
    try {
      const conversation = await ensureSharedConversation();
      const taskId = conversation?.id;
      const chatPayload = buildMailboxChatPayload(text, {
        mention_actor_ids: payload.mentionActorIds,
      });
      if (taskId) {
        const message = await api.sendTeamTaskMessage(props.token, selectedTeamId, taskId, {
          route: "group_chat",
          payload: chatPayload,
        });
        setTaskMessages((prev) =>
          [...prev, message].sort((left, right) => left.message_id - right.message_id)
        );
        await refreshTeamConversationMailboxAfterSend({
          activeRunId: activeRunIdForSelectedTeam,
          taskId,
          refreshSnapshot,
          refreshEvents,
          refreshTaskMessages,
        });
        setTaskMessageDraft("");
        return;
      }
      setWarning("Unable to initialize shared team thread.");
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunIdForSelectedTeam,
    ensureSharedConversation,
    props.token,
    refreshEvents,
    refreshTaskMessages,
    refreshSnapshot,
    selectedTeamId,
    setWarning,
  ]);

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

  useEffect(() => {
    if (
      (tab !== "agent_acp" && tab !== "member_console") ||
      !selectedAgentWorkspaceMemberId
    ) {
      return;
    }
    if (!selectedAgentWorkspaceSessionId) {
      setMemberEvents([]);
      setMemberEventsHasMore(false);
      return;
    }
    void loadMemberEvents("replace");
  }, [
    loadMemberEvents,
    setMemberEventsHasMore,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceSessionId,
    tab,
  ]);
  useEffect(() => {
    if (
      !eventsAutoRefresh ||
      (tab !== "agent_acp" && tab !== "member_console") ||
      !selectedAgentWorkspaceMemberId ||
      !selectedAgentWorkspaceSessionId
    ) {
      return;
    }
    const timer = window.setInterval(() => {
      void loadMemberEvents("replace").catch(() => undefined);
    }, 4000);
    return () => {
      window.clearInterval(timer);
    };
  }, [
    eventsAutoRefresh,
    loadMemberEvents,
    selectedAgentWorkspaceMemberId,
    selectedAgentWorkspaceSessionId,
    tab,
  ]);

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
  }, [setSelectedMemberId, setTab]);
  const onSelectConversationSubject = useCallback(() => {
    setFocusedAgentMemberId("");
    setTab("conversation");
  }, [setTab]);
  const onSelectKanbanSubject = useCallback(() => {
    setFocusedAgentMemberId("");
    setTab("tasks");
  }, [setTab]);
  const onSelectAgentWorkspace = useCallback(
    (memberId: string, nextTab: TeamTab = "agent_acp") => {
      setSelectedMemberId(memberId);
      setFocusedAgentMemberId(memberId);
      setTab(nextTab);
    },
    [setSelectedMemberId, setTab]
  );
  const onSelectUtilityWorkspace = useCallback(
    (nextTab: TeamTab) => {
      setFocusedAgentMemberId("");
      setTab(nextTab);
    },
    [setTab]
  );

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
  const workspaceEyebrow = !selectedTeam
    ? "Team Workbench"
    : isAgentWorkspace
      ? "Agent Workspace"
    : tab === "runs"
      ? "Run Workspace"
      : TEAM_PRIMARY_WORKSPACE_TABS.has(tab)
        ? "Team Workspace"
        : !isAgentWorkspace && tab !== "conversation"
        ? "Team Utility"
        : null;
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
  const selectedAgentStatusView = useMemo(
    () => resolveAgentWorkspaceStatusView(selectedAgentLiveState),
    [selectedAgentLiveState]
  );
  const workspaceTitle = !selectedTeam
    ? "Team Workbench"
    : isAgentWorkspace
      ? selectedAgentLabel
    : tab === "conversation"
      ? selectedConversation?.title ?? DEFAULT_TEAM_THREAD_TITLE
    : tab === "tasks"
      ? "Kanban"
    : tab === "mailbox"
      ? selectedMemberLiveState
        ? selectedAgentLabel
        : "Execution Mailbox"
    : selectedMemberLiveState && isAgentWorkspace
      ? selectedAgentLabel
      : tab === "runs"
        ? "Runs"
        : selectedTeam.name;
  const workspaceDescription = !selectedTeam
    ? "Select a team from the left rail to start team conversations and supervise execution."
    : isAgentWorkspace
      ? "Inspect the selected agent, its direct thread, and its current runtime state."
    : tab === "conversation"
      ? "Shared planning and broadcast thread."
    : tab === "tasks"
        ? "Track agent-owned tasks, linked runs, and the latest execution summaries."
      : tab === "mailbox"
        ? selectedMemberLiveState
          ? "Direct mailbox thread for the selected agent."
          : "Run-scoped mailbox delivery and direct member conversations."
      : tab === "runs"
        ? "Browse runs and choose the active execution context."
        : isAgentWorkspace
          ? "Direct thread for the selected agent."
          : "Operational views stay available without displacing the main thread.";
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
      return [
        selectedAgentLabel,
        selectedAgentStatusView.lifecycle,
        selectedAgentStatusView.work,
        `inbox ${selectedAgentStatusView.inbox}`,
        activeRunForSelectedTeam ? `run ${activeRunForSelectedTeam.status}` : "no active run",
      ]
        .filter((value): value is string => Boolean(value && value !== "unknown"))
        .join(" · ");
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
    selectedAgentLabel,
    selectedAgentStatusView.inbox,
    selectedAgentStatusView.lifecycle,
    selectedAgentStatusView.work,
    selectedTeam,
    selectedTeamMemberLiveStates.length,
    selectedTeamRuntimeStatus.label,
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
  const onOpenRunsWorkspace = useCallback(() => {
    setFocusedAgentMemberId("");
    setTab("runs");
  }, [setTab]);
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
  const onSendAgentAcpInput = useCallback(
    async (text: string, sessionId: string) => {
      const agentId = selectedAgentWorkspaceMemberId;
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
      selectedAgentWorkspaceMemberId,
      selectedTeamId,
      setError,
    ]
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
      await Promise.all([refreshTeams(), refreshAgents()]);
      setTeamRuntimeByTeamId((prev) => {
        const previousRuntime = prev[selectedTeam.id];
        const optimisticRuntime = updateCachedTeamRuntimeStatus(
          previousRuntime,
          selectedTeam.id,
          selectedTeam.name,
          runtime.status as TeamRuntimeRecord["status"],
          runtime.members,
          (sessionStatus) => {
            if (runtime.status !== "running") {
              return sessionStatus ?? undefined;
            }
            return "running";
          }
        );
        if (!optimisticRuntime) {
          return prev;
        }
        return {
          ...prev,
          [selectedTeam.id]: optimisticRuntime,
        };
      });
      void refreshTeamRuntime(selectedTeam.id).catch(() => undefined);
      setWarning(formatTeamRuntimeActionSummary("start", runtime.members));
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    refreshTeams,
    selectedTeamHasConfiguredMembers,
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
      await Promise.all([refreshTeams(), refreshAgents()]);
      setTeamRuntimeByTeamId((prev) => {
        const previousRuntime = prev[selectedTeam.id];
        const optimisticRuntime = updateCachedTeamRuntimeStatus(
          previousRuntime,
          selectedTeam.id,
          selectedTeam.name,
          runtime.status as TeamRuntimeRecord["status"],
          runtime.members,
          null
        );
        if (!optimisticRuntime) {
          return prev;
        }
        return {
          ...prev,
          [selectedTeam.id]: optimisticRuntime,
        };
      });
      void refreshTeamRuntime(selectedTeam.id).catch(() => undefined);
      setWarning(formatTeamRuntimeActionSummary("stop", runtime.members));
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    props.token,
    refreshAgents,
    refreshTeamRuntime,
    refreshTeams,
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
    <div className="space-y-3">
      <TeamTaskPanel
        developerMode={props.developerMode}
        tasksLoading={tasksLoading}
        onRefreshTasks={onRefreshTasks}
        messageDraft={taskMessageDraft}
        onMessageDraftChange={setTaskMessageDraft}
        onSendMessage={onSendTaskMessage}
        onRefreshMessages={refreshTaskMessages}
        messages={taskMessages}
        seenByMessageId={conversationSeenByMessageId}
        humanActorId={HUMAN_MAILBOX_ACTOR_ID}
        memberLiveStates={selectedTeamMemberLiveStates}
        memberIds={taskConversationMemberIds}
        messagesLoading={taskMessagesLoading}
        busy={busy}
        formatTs={formatTs}
        toPrettyJson={toPrettyJson}
      />
    </div>
  );

  const tasksPanel = (
    <TeamTasksPanel
      developerMode={props.developerMode}
      tasks={workspaceTasks}
      tasksLoading={tasksLoading}
      selectedTaskId={selectedTaskId}
      onSelectedTaskIdChange={setSelectedTaskId}
      onRefreshTasks={onRefreshTasks}
      newTaskTitle={newTaskTitle}
      onNewTaskTitleChange={setNewTaskTitle}
      onCreateTask={onCreateTask}
      onUpdateTaskStatus={onUpdateTaskStatus}
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
    />
  );

  const runOpsPanel = (
    <div className="space-y-3">
      <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
        <h4 className={teamSectionHeadingClassName}>Create Run</h4>
        <p className={teamSectionBodyTextClassName}>
          Debug entry for manually starting a Team run.
        </p>
        <div className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-start">
          <TextInput
            className="flex-1"
            radius="md"
            placeholder="context_id (optional, auto-generated when empty)"
            value={runContextId}
            onChange={(event) => setRunContextId(event.target.value)}
          />
          <Button
            radius="md"
            color="dark"
            onClick={onCreateRun}
            disabled={!canCreateRun}
            title={runInputValidation.error ?? teamExecutionBlockedReason ?? "Create run"}
          >
            Create Run
          </Button>
        </div>
        <p className={teamSectionHintTextClassName}>
          <code>context_id</code> can be empty. Use one when you want retries/resume grouped
          under the same context.
        </p>
        <Textarea
          className="mt-3"
          radius="md"
          minRows={8}
          autosize
          placeholder='Optional JSON input, e.g. {"task":"sync"}'
          aria-label="Run input JSON"
          spellCheck={false}
          value={runInput}
          onChange={(event) => setRunInput(event.target.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && canCreateRun) {
              event.preventDefault();
              void onCreateRun();
            }
          }}
          styles={{ input: { fontFamily: "monospace", fontSize: "12px", lineHeight: "1.5" } }}
        />
        {runInputValidation.error ? (
          <p className="mt-2 text-xs text-rose-600" role="alert">
            {runInputValidation.error}
          </p>
        ) : (
          <p className={teamSectionHintTextClassName}>
            {teamExecutionBlockedReason ??
              "Accepts any valid JSON value. Shortcut: Ctrl/Cmd + Enter to create run."}
          </p>
        )}
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <Button
            type="button"
            size="xs"
            radius="md"
            variant="default"
            color="gray"
            onClick={() =>
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
          >
            Use Example JSON
          </Button>
          <Button
            type="button"
            size="xs"
            radius="md"
            variant="default"
            color="gray"
            onClick={() => setRunInput("{}")}
          >
            Set Empty Object
          </Button>
          <Button
            type="button"
            size="xs"
            radius="md"
            variant="default"
            color="gray"
            onClick={() => {
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
            disabled={runInputHasError}
          >
            Format JSON
          </Button>
          <Button
            type="button"
            size="xs"
            radius="md"
            variant="default"
            color="gray"
            onClick={() => setRunInput("")}
            disabled={runInput.trim().length === 0}
          >
            Clear
          </Button>
        </div>
        <p className={teamSectionHintTextClassName}>
          Leave empty to submit default empty input <code>{`{}`}</code>.
        </p>
      </div>
      <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
        <h4 className={teamSectionHeadingClassName}>Load Existing Run</h4>
        <p className={teamSectionBodyTextClassName}>
          Load by <code>run_id</code> for the currently selected team only.
        </p>
        <div className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-start">
          <TextInput
            className="flex-1"
            radius="md"
            placeholder="existing run_id"
            value={runLookupId}
            onChange={(event) => setRunLookupId(event.target.value)}
          />
          <Button
            radius="md"
            variant="default"
            color="gray"
            onClick={onLoadRunById}
            disabled={busy === "load-run"}
            loading={busy === "load-run"}
          >
            Load Run
          </Button>
        </div>
      </div>
    </div>
  );
  const warningNotice = resolveTeamPageNotice(warning);

  return (
    <div className="mx-auto flex h-[var(--agenthub-vh,100vh)] w-full max-w-[1680px] flex-col gap-5 overflow-y-auto overscroll-y-contain bg-[radial-gradient(circle_at_top,_#faf9f6_0%,_#ece8df_45%,_#ddd8cd_100%)] px-3 py-3 sm:px-4 lg:px-6 [&>*]:shrink-0">
      <header className={teamWorkbenchHeaderShellClassName}>
        <div className="flex min-w-0 items-center gap-3">
          {!isSelectorRoute && (
            <button
              className={teamWorkbenchHeaderIconButtonClassName}
              onClick={() => setTeamsSidebarCollapsed((previous) => !previous)}
              title={teamsSidebarCollapsed ? "Show teams panel" : "Hide teams panel"}
              aria-label={teamsSidebarCollapsed ? "Show teams panel" : "Hide teams panel"}
            >
              <i
                className={teamsSidebarCollapsed ? "bi bi-chevron-right" : "bi bi-chevron-left"}
                aria-hidden="true"
              />
            </button>
          )}
          <div className="min-w-0">
            <h1 className="text-[clamp(1.5rem,2.4vw,1.95rem)] font-semibold tracking-tight text-black">
              {isSelectorRoute ? "Team Selector" : selectedTeam?.name ?? "Team Workbench"}
            </h1>
            <p className="mt-1 text-[11px] font-medium uppercase tracking-[0.14em] text-black/55">
              {isSelectorRoute
                ? "Choose a team before entering its workbench"
                : "Mission-first multi-agent workbench"}
            </p>
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-3">
          {!isSelectorRoute && (
            <Button
              type="button"
              size="xs"
              radius="md"
              variant="default"
              className={teamWorkbenchMutedButtonClassName}
              leftSection={<i className="bi bi-grid-3x3-gap" aria-hidden="true" />}
              onClick={navigateToTeamSelector}
            >
              Team Selector
            </Button>
          )}
          <WorkbenchConnectionBadge
            badge={connectionBadge}
            className={teamWorkbenchHeaderStatusClassName}
          />
          <WorkbenchHeaderMenu
            active="teams"
            username={props.auth.username}
            isRoot={props.auth.role === "root"}
            onLogout={props.onLogout}
            onNavigate={navigateTeamRoute}
            buttonClassName={`${teamWorkbenchHeaderIconButtonClassName} h-auto w-auto gap-1.5 px-2 sm:px-3`}
          />
        </div>
      </header>

      {error && <ErrorBanner message={error} onClose={() => setError(null)} />}
      {warningNotice?.kind === "runtime" && (
        <div className={teamRuntimeNoticeClassName} role="status">
          <div className="min-w-0 flex-1">
            <div className={teamRuntimeNoticeTitleClassName}>{warningNotice.title}</div>
            <div className={teamRuntimeNoticeBodyClassName}>{warningNotice.message}</div>
          </div>
          <button
            type="button"
            className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-emerald-200 bg-white/80 text-emerald-700 transition hover:bg-white"
            aria-label="Dismiss runtime notice"
            onClick={() => setWarning(null)}
          >
            <i className="bi bi-x-lg" aria-hidden="true" />
          </button>
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
        <div className="grid min-h-0 flex-1 gap-5 lg:grid-cols-[minmax(360px,440px)_minmax(0,1fr)]">
          <section className={`${teamSectionCardLargeClassName} ${teamWorkbenchPanelClassName}`}>
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="min-w-0 flex-1">
                <span className={teamWorkbenchBadgeClassName}>Team Directory</span>
                <h2 className="mt-2 text-[22px] font-semibold tracking-tight text-black">
                  Select a team
                </h2>
                <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/75">
                  Team selection lives on its own page. Enter a team to open its workspace,
                  members, and operations.
                </p>
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  className={panelSecondaryButtonClassName}
                  onClick={() => {
                    void refreshTeams();
                  }}
                  disabled={busy === "refresh-teams"}
                >
                  Refresh
                </button>
                <Button
                  type="button"
                  radius="md"
                  className={teamWorkbenchAccentButtonClassName}
                  onClick={openCreateTeamModal}
                >
                  Create Team
                </Button>
              </div>
            </div>

            <div className="mt-4">
              <TextInput
                radius="md"
                placeholder="Filter teams by name or id"
                aria-label="Filter teams"
                value={teamSelectorFilter}
                onChange={(event) => setTeamSelectorFilter(event.target.value)}
              />
            </div>

            <div className="mt-4 flex max-h-[55vh] flex-col gap-2 overflow-y-auto pr-1">
              {teams.length === 0 && (
                <p className={teamSectionBodyTextClassName}>
                  No teams yet. Create the team first, then enter its workspace to add agents.
                </p>
              )}
              {teams.length > 0 && selectorVisibleTeams.length === 0 && (
                <p className={teamSectionBodyTextClassName}>No teams match the current filter.</p>
              )}
              {selectorVisibleTeams.map((team) => {
                const summary = teamMemberSummaryByTeamId.get(team.id);
                const summaryLabel = summary
                  ? `${summary.total} members · ${summary.active} active${
                      summary.inactive > 0 ? ` · ${summary.inactive} idle` : ""
                    }${summary.missing > 0 ? ` · ${summary.missing} missing` : ""}`
                  : "No agents configured yet";
                return (
                  <button
                    key={team.id}
                    type="button"
                    className="team-item flex w-full min-w-0 flex-col items-start gap-1 rounded-[14px] border border-ui-border bg-ui-surface px-3 py-3 text-left shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft"
                    onClick={() => navigateToTeamDetail(team.id)}
                  >
                    <span className="flex w-full items-start justify-between gap-3">
                      <span className="truncate text-[15px] font-semibold text-ui-text-primary">
                        {team.name}
                      </span>
                      <span className="shrink-0 rounded-full border border-ui-border bg-ui-surface-soft px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted">
                        Open
                      </span>
                    </span>
                    <span className="text-[12px] leading-5 text-ui-text-secondary">
                      {team.description?.trim() || "Open this team workspace to manage members and runs."}
                    </span>
                    <span className="mono text-ui-xs text-ui-text-muted">{summaryLabel}</span>
                    {props.developerMode && (
                      <span className="mono text-ui-xs text-ui-text-muted/90">{team.id}</span>
                    )}
                  </button>
                );
              })}
            </div>
          </section>

          <section className={`${teamSectionCardLargeClassName} ${teamWorkbenchPanelClassName}`}>
            <span className={teamWorkbenchBadgeClassName}>Goal First</span>
            <h2 className="mt-2 text-[22px] font-semibold tracking-tight text-black">
              Create the team before you hire the agents.
            </h2>
            <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/75">
              Start with the mission. A new team stays empty until you add the first agent and
              the rest of the agents with their role, skills, and prompt profile.
            </p>
            <Group gap="sm" mt="md">
              <Button
                type="button"
                radius="md"
                className={teamWorkbenchAccentButtonClassName}
                onClick={openCreateTeamModal}
              >
                Create Team
              </Button>
            </Group>
          </section>
        </div>
      ) : (
        <div
          className={
            teamsSidebarCollapsed
              ? "teams-layout grid min-h-0 flex-1 gap-5 lg:grid-cols-[minmax(0,1fr)]"
              : "teams-layout grid min-h-0 flex-1 gap-5 lg:grid-cols-[minmax(320px,380px)_minmax(0,1fr)]"
          }
        >
          {!teamsSidebarCollapsed && (
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
              selectedTeamId={selectedTeamId}
              teamMemberSummaryByTeamId={teamMemberSummaryByTeamId}
              memberLiveStates={selectedTeamMemberLiveStates}
              focusedAgentMemberId={focusedAgentMemberId}
              tab={tab}
              onSelectTeam={(teamId) => {
                if (teamId !== selectedTeamId) {
                  navigateToTeamDetail(teamId);
                }
              }}
              onSelectConversation={onSelectConversationSubject}
              onSelectKanban={onSelectKanbanSubject}
              onSelectAgentTab={onSelectAgentWorkspace}
              onSelectUtilityTab={onSelectUtilityWorkspace}
            />
          )}

          <div className="teams-main flex min-h-0 min-w-0 flex-col gap-5 overflow-y-auto pb-2 pr-1 [&>*]:shrink-0">
            {!selectedTeam && (
              <div className={`${teamSectionCardLargeClassName} ${teamWorkbenchPanelClassName}`}>
                <span className={teamWorkbenchBadgeClassName}>Team Not Found</span>
                <h2 className="mt-2 text-[22px] font-semibold tracking-tight text-black">
                  This team is unavailable.
                </h2>
                <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/75">
                  The requested team could not be loaded. Return to the selector to choose another
                  team or create a new one.
                </p>
                <Group gap="sm" mt="md">
                  <Button
                    type="button"
                    radius="md"
                    className={teamWorkbenchAccentButtonClassName}
                    onClick={navigateToTeamSelector}
                  >
                    Back to Team Selector
                  </Button>
                </Group>
              </div>
            )}

            {selectedTeam && (
              <>
                <div className={`${teamSectionCardClassName} ${teamWorkbenchWorkspaceShellClassName}`}>
                <div className="flex flex-col gap-4">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="min-w-0 flex-1">
                    {workspaceEyebrow && (
                      <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-black/58">
                        {workspaceEyebrow}
                      </p>
                    )}
                    <h2 className={`${workspaceEyebrow ? "mt-1" : ""} text-[18px] font-semibold tracking-tight text-black`}>
                      {workspaceTitle}
                    </h2>
                    <p className={teamSectionBodyTextClassName}>{workspaceDescription}</p>
                    {isAgentWorkspace ? (
                      <div className="mt-3 max-w-4xl border-t border-ui-border pt-3">
                        <div className="grid gap-px overflow-hidden rounded-[16px] border border-ui-border bg-ui-border sm:grid-cols-2 xl:grid-cols-4">
                          {[
                            { label: "Role", value: selectedAgentStatusView.role },
                            { label: "Lifecycle", value: selectedAgentStatusView.lifecycle },
                            { label: "Work", value: selectedAgentStatusView.work },
                            { label: "Inbox", value: selectedAgentStatusView.inbox },
                          ].map((item) => (
                            <div key={item.label} className={teamWorkbenchInfoStripItemClassName}>
                              <p className={teamWorkbenchInfoStripLabelClassName}>{item.label}</p>
                              <p className={teamWorkbenchInfoStripValueClassName}>{item.value}</p>
                            </div>
                          ))}
                        </div>
                        <div className="mt-3 rounded-[14px] border border-ui-border bg-ui-surface px-3.5 py-3">
                          <p className={teamWorkbenchInfoStripLabelClassName}>Current Work</p>
                          <p className="mt-1.5 text-[13px] leading-5 text-ui-text-secondary">
                            {selectedAgentStatusView.currentWork}
                          </p>
                        </div>
                      </div>
                    ) : selectedTeam.description?.trim() ? (
                      <div className="mt-3 max-w-3xl border-t border-ui-border pt-3">
                        <p className={teamWorkbenchInfoStripLabelClassName}>Goal</p>
                        <p className="mt-1.5 text-[13px] leading-5 text-ui-text-secondary">
                          {selectedTeam.description.trim()}
                        </p>
                      </div>
                    ) : null}
                  </div>
                  {!isAgentWorkspace && (
                  <div className="flex flex-wrap items-center gap-2">
                    <Tooltip
                      label={`${selectedTeamRuntimeStatus.online}/${selectedTeamRuntimeStatus.total} members online`}
                      withArrow
                    >
                      <Group
                        gap={8}
                        wrap="nowrap"
                        className="rounded-[14px] border border-ui-border bg-ui-surface px-2.5 py-1.5 shadow-sm"
                      >
                        <Badge
                          variant="light"
                          color={selectedTeamRuntimeControlTone.statusColor}
                          radius="sm"
                        >
                          {selectedTeamRuntimeStatus.label}
                        </Badge>
                        <Badge
                          variant="dot"
                          color={selectedTeamRuntimeControlTone.countColor}
                          radius="sm"
                        >
                          {`${selectedTeamRuntimeStatus.online}/${selectedTeamRuntimeStatus.total} online`}
                        </Badge>
                      </Group>
                    </Tooltip>
                    <div className={teamWorkbenchHeaderActionsClassName}>
                      <Button
                        type="button"
                        size="xs"
                        radius="md"
                        className={`${teamWorkbenchAccentButtonClassName} ${teamWorkbenchHeaderActionButtonClassName}`}
                        leftSection={<i className="bi bi-person-plus" aria-hidden="true" />}
                        onClick={openTeamMemberForgeModal}
                      >
                        {teamMemberForgeLabel}
                      </Button>
                      <Button
                        type="button"
                        size="xs"
                        radius="md"
                        className={`${teamWorkbenchAccentButtonClassName} ${teamWorkbenchHeaderActionButtonClassName}`}
                        loading={busy === "start-team"}
                        disabled={
                          busy === "stop-team" ||
                          selectedTeamRuntimeStatus.status === "running" ||
                          !selectedTeamHasConfiguredMembers
                        }
                        title={teamExecutionBlockedReason ?? "Start team runtime"}
                        leftSection={<i className="bi bi-play-circle" aria-hidden="true" />}
                        onClick={onStartTeamRuntime}
                      >
                        Start Team
                      </Button>
                      <Button
                        type="button"
                        size="xs"
                        radius="md"
                        variant="default"
                        className={`${teamWorkbenchMutedButtonClassName} ${teamWorkbenchHeaderActionButtonClassName}`}
                        loading={busy === "stop-team"}
                        disabled={
                          busy === "start-team" || selectedTeamRuntimeStatus.status === "stopped"
                        }
                        leftSection={<i className="bi bi-stop-circle" aria-hidden="true" />}
                        onClick={onStopTeamRuntime}
                      >
                        Stop Team
                      </Button>
                    </div>
                  </div>
                  )}
                </div>
                <div className="flex flex-wrap items-center justify-between gap-3 border-t border-ui-border pt-3">
                  {!isAgentWorkspace ? (
                    <Tabs
                      value={primaryWorkspaceTabValue}
                      onChange={(value) => {
                        if (value) {
                          setFocusedAgentMemberId("");
                          setTab(value as TeamTab);
                        }
                      }}
                      classNames={{
                        list: workspacePrimaryTabsListClassName,
                        tab: workspacePrimaryTabClassName,
                      }}
                    >
                      <Tabs.List>
                        {TEAM_PRIMARY_WORKSPACE_ITEMS.map((item) => (
                          <Tabs.Tab key={item.value} value={item.value}>
                            <span className="inline-flex items-center gap-1.5">
                              <i className={item.icon} aria-hidden="true" />
                              <span>{item.label}</span>
                            </span>
                          </Tabs.Tab>
                        ))}
                      </Tabs.List>
                    </Tabs>
                  ) : (
                    <div className="min-h-9" />
                  )}
                  <div className={workspaceToolbarClassName}>
                      <button
                        type="button"
                        className={
                          tab === "runs"
                            ? workspaceToolbarButtonActiveClassName
                            : workspaceToolbarButtonIdleClassName
                        }
                        onClick={onOpenRunsWorkspace}
                      >
                        <i className="bi bi-play-circle" aria-hidden="true" />
                        <span>Runs</span>
                      </button>
                      {(workspaceAdvancedTabItems.length > 0 || showRunActionsInAdvanced) && (
                        <Menu withinPortal={false} position="bottom-end" shadow="md">
                          <Menu.Target>
                            <button
                              type="button"
                              className={
                                isAdvancedWorkspace
                                  ? workspaceToolbarButtonActiveClassName
                                  : workspaceToolbarButtonIdleClassName
                              }
                              aria-label="Open advanced views"
                            >
                              <i className="bi bi-sliders2" aria-hidden="true" />
                              <span>Advanced</span>
                            </button>
                          </Menu.Target>
                          <Menu.Dropdown>
                            {workspaceAdvancedTabItems.length > 0 && (
                              <>
                                <Menu.Label>Views</Menu.Label>
                                {workspaceAdvancedTabItems.map((item) => (
                                  <Menu.Item
                                    key={item.value}
                                    onClick={() => setTab(item.value)}
                                  >
                                    {item.label}
                                  </Menu.Item>
                                ))}
                              </>
                            )}
                            {showRunActionsInAdvanced && (
                              <>
                                {workspaceAdvancedTabItems.length > 0 && <Menu.Divider />}
                                <Menu.Label>Run</Menu.Label>
                                <Menu.Item onClick={onRefreshActiveRun}>
                                  Refresh Run
                                </Menu.Item>
                                <Menu.Item
                                  onClick={onCancelRun}
                                  disabled={
                                    busy === "cancel-run" ||
                                    activeRunForSelectedTeam.status === "canceled"
                                  }
                                >
                                  Cancel
                                </Menu.Item>
                                <Menu.Item
                                  onClick={onResumeRun}
                                  disabled={busy === "resume-run" || !canResumeActiveRun}
                                >
                                  Resume
                                </Menu.Item>
                                <Menu.Item
                                  onClick={onRestartRun}
                                  disabled={busy === "restart-run" || !canRestartActiveRun}
                                >
                                  Restart
                                </Menu.Item>
                              </>
                            )}
                          </Menu.Dropdown>
                        </Menu>
                      )}
                  </div>
                </div>
                <div className={workspaceNoticeClassName}>
                  <div className={workspaceNoticeTextClassName}>
                    <span className={workspaceNoticeDotClassName} aria-hidden="true" />
                    <span className="min-w-0 flex-1 text-xs leading-5 text-ui-text-muted">
                      {workspaceNoticeText}
                    </span>
                  </div>
                  {props.developerMode && (
                    <div className="relative">
                      <button
                        type="button"
                        className="inline-flex h-9 w-9 items-center justify-center rounded-[12px] border border-ui-border bg-white text-ui-text-primary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft"
                        onClick={() => setWorkspaceDetailsOpen((current) => !current)}
                        aria-expanded={workspaceDetailsOpen}
                        aria-label="Toggle workspace details"
                        title="Workspace details"
                      >
                        <i className="bi bi-three-dots" aria-hidden="true" />
                      </button>
                      {workspaceDetailsOpen && (
                        <div className={workspaceMetaDropdownClassName}>
                          {workspaceDetailItems.map((item) => (
                            <div
                              key={item}
                              className={teamRunMetaItemClassName}
                            >
                              {item}
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
                </div>
              </div>

              {!selectedTeamHasConfiguredMembers && (
                <div className={`${TEAM_CREATE_PANEL_CARD_CLASS} ${teamWorkbenchPanelClassName}`}>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <span className={teamWorkbenchBadgeClassName}>Team Setup</span>
                      <h3 className="mt-2 text-[18px] font-semibold tracking-tight text-black">
                        No agents have joined this team yet.
                      </h3>
                      <p className="mt-2 max-w-2xl text-[13px] leading-5 text-ui-text-secondary">
                        The team goal is saved, but runtime and runs stay blocked until you add the
                        first agent.
                      </p>
                    </div>
                    <Button
                      type="button"
                      radius="md"
                      className={teamWorkbenchAccentButtonClassName}
                      leftSection={<i className="bi bi-person-plus" aria-hidden="true" />}
                      onClick={openTeamMemberForgeModal}
                    >
                      {teamMemberForgeLabel}
                    </Button>
                  </div>
                  <div className={`${teamWorkbenchSetupChecklistClassName} mt-4`}>
                    <div className={teamWorkbenchInfoStripGridClassName}>
                      <div className={teamWorkbenchInfoStripItemClassName}>
                        <p className={teamWorkbenchInfoStripLabelClassName}>Goal</p>
                        <p className={teamWorkbenchInfoStripValueClassName}>
                          {selectedTeam.description?.trim() ||
                            "Capture the mission, constraints, and what this team should own."}
                        </p>
                      </div>
                      <div className={teamWorkbenchInfoStripItemClassName}>
                        <p className={teamWorkbenchInfoStripLabelClassName}>First Agent</p>
                        <p className={teamWorkbenchInfoStripValueClassName}>
                          Add the first agent with identity, skills, prompt, and workdir.
                        </p>
                      </div>
                      <div className={teamWorkbenchInfoStripItemClassName}>
                        <p className={teamWorkbenchInfoStripLabelClassName}>Unlocks</p>
                        <p className={teamWorkbenchInfoStripValueClassName}>
                          Runtime, runs, and shared execution views unlock automatically once an
                          agent exists.
                        </p>
                      </div>
                    </div>
                    <div className="grid gap-px border-t border-ui-border bg-ui-border lg:grid-cols-3">
                      <div className={teamWorkbenchInfoStripItemClassName}>
                        <p className={teamWorkbenchInfoStripLabelClassName}>Step 1</p>
                        <p className={teamWorkbenchInfoStripValueClassName}>Create the first agent</p>
                      </div>
                      <div className={teamWorkbenchInfoStripItemClassName}>
                        <p className={teamWorkbenchInfoStripLabelClassName}>Step 2</p>
                        <p className={teamWorkbenchInfoStripValueClassName}>Add more agents</p>
                      </div>
                      <div className={teamWorkbenchInfoStripItemClassName}>
                        <p className={teamWorkbenchInfoStripLabelClassName}>Step 3</p>
                        <p className={teamWorkbenchInfoStripValueClassName}>Start runtime and runs</p>
                      </div>
                    </div>
                  </div>
                </div>
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
                  pageLimit={TEAM_RUN_PAGE_LIMIT}
                  runsHasMore={runsHasMore}
                  selectedTeamId={selectedTeamId}
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
                  <h3 className={teamSectionTitleClassName}>No Active Run</h3>
                  <p className={teamSectionBodyTextClassName}>
                    Select an existing run or start one in the Runs tab before opening this panel.
                  </p>
                  <div className="mt-3">
                    <button
                      className={panelSecondaryButtonClassName}
                      type="button"
                      onClick={() => setTab("runs")}
                    >
                      Go to Runs
                    </button>
                  </div>
                </div>
              )}

              {tab !== "runs" && !showRunContextLoading && !showNoActiveRunNotice && (
                <div className="flex min-w-0 flex-col gap-3">
                  {tab === "conversation" && (
                    <>
                      {!activeRunForSelectedTeam && (
                        <div className={teamSectionCardClassName}>
                          <p className={teamSectionBodyTextClassName}>
                            The shared thread is available before execution starts.
                          </p>
                        </div>
                      )}
                      {conversationPanel}
                    </>
                  )}

                  {tab === "tasks" && tasksPanel}

                  {tab === "agent_acp" && (
                    <TeamMemberAcpPanel
                      developerMode={props.developerMode}
                      selectedMemberId={selectedAgentWorkspaceMemberId}
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
                      onRefresh={onRefreshMemberConsole}
                      onLoadOlder={onLoadOlderMemberConsole}
                    />
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
                      onRefreshSteps={async () => {
                        await refreshSteps(activeRunForSelectedTeam.id);
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

                  {tab === "mailbox" && !activeRunForSelectedTeam && (
                    <div className={teamSectionCardClassName}>
                      <h3 className={teamSectionTitleClassName}>
                        {isAgentWorkspace ? selectedAgentLabel : "Execution Mailbox"}
                      </h3>
                      <p className={teamSectionBodyTextClassName}>
                        {isAgentWorkspace
                          ? "This agent is selected, but there is no active run context for its direct thread yet. Use Runs to inspect execution history or wait for the next task."
                          : "Execution mailbox is run-scoped. Start or select a run to inspect delivery and direct member conversations."}
                      </p>
                      <div className="mt-3">
                        <button
                          className={panelSecondaryButtonClassName}
                          type="button"
                          onClick={() => setTab("runs")}
                        >
                          Go to Runs
                        </button>
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
                      <div className={`${TEAM_PANEL_CARD_CLASS} p-3`}>
                        <div className="flex flex-wrap items-center justify-between gap-3">
                          <h3 className={teamSectionHeadingClassName}>Debug Tools</h3>
                          <div className={teamDebugTabsClassName}>
                            <button
                              className={
                                teamDebugTag === "run_ops"
                                  ? teamDebugTabActiveClassName
                                  : teamDebugTabIdleClassName
                              }
                              onClick={() => setTeamDebugTag("run_ops")}
                            >
                              Run Ops
                            </button>
                            <button
                              className={
                                teamDebugTag === "step_ops"
                                  ? teamDebugTabActiveClassName
                                  : teamDebugTabIdleClassName
                              }
                              onClick={() => setTeamDebugTag("step_ops")}
                            >
                              Step Ops
                            </button>
                            <button
                              className={
                                teamDebugTag === "mailbox_raw"
                                  ? teamDebugTabActiveClassName
                                  : teamDebugTabIdleClassName
                              }
                              onClick={() => setTeamDebugTag("mailbox_raw")}
                            >
                              Mailbox Raw
                            </button>
                          </div>
                        </div>
                      </div>

                      {teamDebugTag === "run_ops" && runOpsPanel}

                      {teamDebugTag === "step_ops" && !activeRunForSelectedTeam && (
                        <div className={teamSectionCardClassName}>
                          <h4 className={teamSectionHeadingClassName}>Step Ops</h4>
                          <p className={teamSectionBodyTextClassName}>
                            Step operations require an active run. Start or select one in the Runs
                            tab first.
                          </p>
                          <div className="mt-3">
                            <button
                              className={panelSecondaryButtonClassName}
                              type="button"
                              onClick={() => setTab("runs")}
                            >
                              Go to Runs
                            </button>
                          </div>
                        </div>
                      )}

                      {teamDebugTag === "step_ops" && activeRunForSelectedTeam && (
                        <TeamStepsPanel
                          developerMode={props.developerMode}
                          mode="controls_only"
                          steps={steps}
                          onRefreshSteps={async () => {
                            await refreshSteps(activeRunForSelectedTeam.id);
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

                      {teamDebugTag === "mailbox_raw" && !activeRunForSelectedTeam && (
                        <div className={teamSectionCardClassName}>
                          <h4 className={teamSectionHeadingClassName}>Mailbox Raw</h4>
                          <p className={teamSectionBodyTextClassName}>
                            Mailbox raw operations require an active run. Start or select one in
                            the Runs tab first.
                          </p>
                          <div className="mt-3">
                            <button
                              className={panelSecondaryButtonClassName}
                              type="button"
                              onClick={() => setTab("runs")}
                            >
                              Go to Runs
                            </button>
                          </div>
                        </div>
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
                    </>
                  )}
                </div>
              )}
            </>
          )}
          </div>
        </div>
      )}

      {showCreateTeamModal && (
        <div
          className={TEAM_CREATE_MODAL_BACKDROP_CLASS}
          role="presentation"
          onClick={(event) => {
            if (event.target === event.currentTarget && busy !== "create-team") {
              closeCreateTeamModal();
            }
          }}
        >
          <div
            className={`${TEAM_CREATE_MODAL_CARD_CLASS} ${teamWorkbenchPanelClassName}`}
            role="dialog"
            aria-modal="true"
            aria-labelledby="team-create-title"
          >
            <div className={teamCreateModalHeaderClassName}>
              <div className="min-w-0 flex-1">
                <span className={teamWorkbenchBadgeClassName}>Create Team</span>
                <h3 id="team-create-title" className="mt-2 text-[18px] font-semibold tracking-tight text-black">
                  Start with the mission, not the agents.
                </h3>
                <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/70">
                  Team creation only stores the workspace identity and goal. Add agents afterward,
                  each with their own role profile, skills, and prompt.
                </p>
              </div>
            </div>
            <div className="modal-body mt-4 space-y-4">
              <div className={TEAM_CREATE_PANEL_CARD_CLASS}>
                <TextInput
                  label="Team name"
                  radius="md"
                  placeholder="growth-hive"
                  value={newTeamName}
                  onChange={(event) => setNewTeamName(event.target.value)}
                />
                <Textarea
                  className="mt-3"
                  label="Team goal"
                  radius="md"
                  minRows={5}
                  autosize
                  placeholder="Describe the mission, constraints, and what this team should own."
                  value={newTeamDescription}
                  onChange={(event) => setNewTeamDescription(event.target.value)}
                />
              </div>

              <TeamCreateNote tone={newTeamName.trim() ? "info" : "warning"}>
                {newTeamName.trim()
                  ? "After the team is created, add the first agent. More agents can be added after the first agent exists."
                  : "Team name is required before the team can be created."}
              </TeamCreateNote>
            </div>

            <div className={TEAM_CREATE_ACTIONS_BAR_CLASS}>
              <Button
                radius="md"
                variant="default"
                className={teamWorkbenchMutedButtonClassName}
                onClick={closeCreateTeamModal}
                disabled={busy === "create-team"}
                type="button"
              >
                Cancel
              </Button>
              <Button
                radius="md"
                className={teamWorkbenchAccentButtonClassName}
                onClick={onCreateTeam}
                disabled={busy === "create-team" || !newTeamName.trim()}
                loading={busy === "create-team"}
                type="button"
              >
                Create Team
              </Button>
            </div>
          </div>
        </div>
      )}

      {showForgeAgentForm && teamMemberDraft && (
        <CreateAgentModal
          title="Add Agent"
          confirmLabel="Create Agent"
          agentPresetLabel="Role model"
          agentPresetSummaryLabel="Model"
          teamStyled
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
          showWorktreeAdvancedOptions={teamMemberDraft.role !== "leader"}
          createBusy={forgeAgentBusy}
          workdirPlaceholder={forgeDefaultWorktreeRoot}
          withinPortal
          onCreateAgent={onCreateForgeAgent}
          onClose={closeTeamMemberForgeModal}
        >
          <div className={`${TEAM_CREATE_PANEL_CARD_CLASS} border border-ui-border bg-ui-surface/90`}>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className={teamWorkbenchBadgeClassName}>
                {teamMemberRoleProfile?.profileLabel ?? "Agent Profile"}
              </span>
              <span className="text-xs font-medium uppercase tracking-[0.14em] text-ui-text-muted">
                member_id follows agent id
              </span>
            </div>
            <p className="mt-2 text-[13px] leading-5 text-ui-text-secondary">
              {teamMemberRoleProfile?.intro ??
                "Configure the agent identity, skills, and prompt before attaching it to the team."}
            </p>
            <div className="mt-4 rounded-[14px] border border-ui-border bg-ui-surface-soft px-3.5 py-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="min-w-0">
                  <p className={teamWorkbenchInfoStripLabelClassName}>Role Selection</p>
                  <p className="mt-1 text-[12px] leading-5 text-ui-text-secondary">
                    {selectedTeamHasLeader
                      ? "This team already has a leader. New agents join as workers."
                      : "Start with the leader. Worker unlocks after the first leader exists."}
                  </p>
                </div>
                <span className={teamWorkbenchBadgeClassName}>
                  {teamMemberDraft.role === "leader" ? "Single leader" : "Execution role"}
                </span>
              </div>
              <SegmentedControl
                className="mt-3"
                fullWidth
                radius="xl"
                size="sm"
                value={teamMemberDraft.role}
                onChange={handleTeamMemberRoleChange}
                data={teamMemberRoleOptions.map((option) => ({
                  value: option.value,
                  label: option.label,
                  disabled: option.disabled,
                }))}
              />
              <p className="mt-2 text-[11px] leading-5 text-ui-text-muted">
                {teamMemberRoleOptions.find((option) => option.value === teamMemberDraft.role)
                  ?.description ?? "Select the role before editing skills and prompt."}
              </p>
            </div>
            <div className={`${teamWorkbenchSetupChecklistClassName} mt-4`}>
              <div className={teamWorkbenchInfoStripGridClassName}>
                <div className={teamWorkbenchInfoStripItemClassName}>
                  <p className={teamWorkbenchInfoStripLabelClassName}>Focus</p>
                  <p className={teamWorkbenchInfoStripValueClassName}>
                    {teamMemberRoleProfile?.focus ??
                      "Set the role before editing the profile details."}
                  </p>
                </div>
                <div className={teamWorkbenchInfoStripItemClassName}>
                  <p className={teamWorkbenchInfoStripLabelClassName}>Skills</p>
                  <p className={teamWorkbenchInfoStripValueClassName}>
                    {teamMemberRoleProfile?.skillsHint ??
                      "Select required skills first, then add optional helpers."}
                  </p>
                </div>
                <div className={teamWorkbenchInfoStripItemClassName}>
                  <p className={teamWorkbenchInfoStripLabelClassName}>Prompt Scope</p>
                  <p className={teamWorkbenchInfoStripValueClassName}>
                    {teamMemberRoleProfile?.promptHint ??
                      "Keep the role prompt focused on scope, responsibilities, and delivery rules."}
                  </p>
                </div>
              </div>
            </div>

            <TextInput
              className="mt-4"
              radius="md"
              label="Identity"
              placeholder="Short role description exposed on the agent card"
              value={teamMemberDraft.description}
              onChange={(event) =>
                patchTeamMemberDraft({ description: event.currentTarget.value })
              }
            />
            <div className="team-skill-tags mt-4 flex flex-wrap gap-2">
              {TEAM_SKILL_OPTIONS.map((skill) => {
                const selected = teamMemberDraft.skills.includes(skill);
                const requiredSkills =
                  teamMemberDraft.role === "leader"
                    ? REQUIRED_TEAM_LEADER_SKILLS
                    : REQUIRED_TEAM_WORKER_SKILLS;
                const isRequired = requiredSkills.includes(skill);
                return (
                  <button
                    key={`${teamMemberDraft.role}-skill-${skill}`}
                    type="button"
                    className={
                      selected
                        ? TEAM_CREATE_SKILL_TAG_SELECTED_CLASS
                        : TEAM_CREATE_SKILL_TAG_IDLE_CLASS
                    }
                    onClick={() =>
                      patchTeamMemberDraft({
                        skills: toggleSkillSelection(
                          teamMemberDraft.skills,
                          skill,
                          requiredSkills
                        ),
                      })
                    }
                    disabled={isRequired}
                    title={isRequired ? "Required for this role" : undefined}
                  >
                    {skill}
                  </button>
                );
              })}
            </div>
            <TextInput
              className="mt-3"
              radius="md"
              label="Custom skills"
              placeholder="comma separated, optional"
              value={teamMemberDraft.custom_skills}
              onChange={(event) =>
                patchTeamMemberDraft({ custom_skills: event.currentTarget.value })
              }
            />
            <Textarea
              className="mt-3"
              radius="md"
              label="Prompt"
              minRows={6}
              autosize
              value={teamMemberDraft.prompt}
              onChange={(event) =>
                patchTeamMemberDraft({ prompt: event.currentTarget.value })
              }
              styles={{
                input: {
                  fontFamily: "monospace",
                  fontSize: "12px",
                  lineHeight: "1.5",
                },
              }}
            />
          </div>
        </CreateAgentModal>
      )}
    </div>
  );
}
