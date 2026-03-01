import React, { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  AgentDiscoveryCardRecord,
  AgentRecord,
  AgentEvent,
  api,
  TeamConversationMessageRecord,
  TeamActorMessageRecord,
  TeamDefinitionRecord,
  TeamMainTaskRecord,
  TeamMainTaskRunCompilePreviewRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamRunSnapshotRecord,
  TeamStepRecord,
} from "../api";
import {
  DEFAULT_AGENT_PRESET_ID,
  getAgentPreset,
  type AgentPresetId,
} from "../agent_presets";
import { CreateAgentModal } from "../components/create_agent_modal";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";
import {
  normalizeWorkdirInput,
  resolveWorkdirForModeChange,
  resolveWorkdirForModalOpen,
} from "../worktree_defaults";
import { TeamEventsPanel } from "./team_events_panel";
import { TeamMailboxPanel } from "./team_mailbox_panel";
import { TeamMemberAcpPanel } from "./team_member_acp_panel";
import { TeamActiveRunPanel } from "./team_active_run_panel";
import { TeamMemberConsolePanel } from "./team_member_console_panel";
import { TeamMemberStatusStrip } from "./team_member_status_strip";
import { TeamMainTaskPanel } from "./team_main_task_panel";
import { TeamOverviewPanel } from "./team_overview_panel";
import { TeamRunPanel } from "./team_run_panel";
import { TeamSidebar } from "./team_sidebar";
import { TeamStepsPanel } from "./team_steps_panel";
import { TeamTabsBar } from "./team_tabs_bar";
import {
  buildTeamForgeCleanupWarning,
  buildLeaderForgeDefaultWorkdir,
  buildTeamSpecFromForm,
  clampCreateTeamStage,
  cleanupUnusedTeamForgeAgents,
  formatTeamForgeWorktreeError,
  parseErrorMessage,
  parseRequiredJson,
  resolveUnusedTeamForgeAgentIds,
  resolveTeamModelOptions,
} from "./team/create_helpers";
import {
  clearTeamCreateDraft,
  loadTeamCreateDraft,
  persistTeamCreateDraft,
  type TeamCreateEntryMode,
} from "./team/create_draft_storage";
import {
  MailboxTemplateKey,
  buildMailboxChatPayload,
  buildMailboxConversationKey,
  buildMailboxPayloadTemplate,
  countUnreadConversationMessages,
  extractMentionedActorIds,
  mergeMailboxMessages,
  resolveMainTaskMailboxRoutePlan,
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
  buildDefaultWorkerDraft,
  createInitialTeamDraftState,
  parseTeamSpecMembers,
  resolveTeamMemberAgentStatuses,
  selectTeamForgeAgents,
  summarizeTeamMemberAgentStatuses,
  toggleSkillSelection,
  type WorkerDraft,
  assignCreatedWorkerToDraft,
} from "./team/member_helpers";
import {
  buildAgentLabel,
  formatTs,
  pickNextWorkerAgentId,
  toPrettyJson,
  upsertRun,
} from "./team/page_helpers";
import {
  selectTeamPreviewEvents,
  type TeamRunStatusFilter,
} from "./team/run_helpers";
import { useTeamCreateModalLifecycleEffects } from "./team/use_team_create_modal_lifecycle_effects";
import { useTeamActions } from "./team/use_team_actions";
import { useTeamMailboxActions } from "./team/use_team_mailbox_actions";
import { useTeamMemberAgentBackfillEffect } from "./team/use_team_member_agent_backfill_effect";
import { useTeamMailboxLifecycleEffects } from "./team/use_team_mailbox_lifecycle_effects";
import { useTeamRunLifecycleEffects } from "./team/use_team_run_lifecycle_effects";
import { useTeamStepActions } from "./team/use_team_step_actions";
import {
  CREATE_TEAM_STAGE_TITLES,
  DEFAULT_TEAM_CONTROL_STATE,
  DEFAULT_TEAM_MAILBOX_STATE,
  DEFAULT_TEAM_RUN_BROWSER_STATE,
  DEFAULT_TEAM_UI_STATE,
  DEFAULT_WORKTREE_ROOT,
  MAILBOX_TEMPLATE_OPTIONS,
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
  type CreateTeamStage,
  type StepAction,
  type TeamControlState,
  type TeamCreateState,
  type TeamForgeRoleTag,
  type TeamMailboxState,
  type TeamRunBrowserState,
} from "./team/state";
import {
  TEAM_CREATE_ACTIONS_BAR_CLASS,
  TEAM_CREATE_MODAL_BACKDROP_CLASS,
  TEAM_CREATE_MODAL_CARD_CLASS,
  TEAM_CREATE_NOTE_INFO_CLASS,
  TEAM_CREATE_NOTE_WARNING_CLASS,
  TEAM_CREATE_PANEL_CARD_CLASS,
  TEAM_CREATE_SKILL_TAG_IDLE_CLASS,
  TEAM_CREATE_SKILL_TAG_SELECTED_CLASS,
  TEAM_CREATE_STAGE_BADGE_CLASS,
  TEAM_CREATE_STEP_PREVIEW_CLASS,
  TEAM_CREATE_STEP_PREVIEW_MUTED_CLASS,
  TEAM_CREATE_WORKER_CARD_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_GHOST_BUTTON_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
} from "../ui/tailwind_classes";

export {
  buildMailboxChatPayload,
  buildMailboxConversationKey,
  buildMailboxPayloadTemplate,
  countUnreadConversationMessages,
  extractMentionedActorIds,
  mergeMailboxMessages,
  resolveMainTaskMailboxRoutePlan,
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
  selectTeamPreviewEvents,
} from "./team/run_helpers";
export type { MailboxTemplateKey, TeamMailboxChatActors } from "./team/mailbox_helpers";
export type {
  TeamCreateDraftState,
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
};
type TeamDebugTag = "run_ops" | "step_ops" | "mailbox_raw";

const TEAM_EVENT_PREVIEW_LIMIT = 5;

type RunInputValidation = {
  parsed: unknown | undefined;
  error: string | null;
};

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

function sortMainTasksByActivity(tasks: TeamMainTaskRecord[]): TeamMainTaskRecord[] {
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

function buildAutoConversationTitle(messageText: string): string {
  const normalized = messageText.replace(/\s+/g, " ").trim();
  if (normalized.length > 0) {
    return normalized.slice(0, 72);
  }
  return "Team conversation";
}

function asObjectRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function resolveTeamLeaderMemberId(spec: unknown, memberIds: string[]): string {
  const normalizedMemberIds = [...new Set(memberIds.map((memberId) => memberId.trim()).filter(Boolean))];
  if (normalizedMemberIds.length === 0) {
    return "";
  }
  const memberSet = new Set(normalizedMemberIds);
  const specRecord = asObjectRecord(spec);
  const leaderFromSpec = typeof specRecord?.leader_member_id === "string"
    ? specRecord.leader_member_id.trim()
    : "";
  if (leaderFromSpec && memberSet.has(leaderFromSpec)) {
    return leaderFromSpec;
  }
  const leaderFromRole =
    parseTeamSpecMembers(spec).find((member) => member.role.trim().toLowerCase() === "leader")
      ?.member_id ?? "";
  if (leaderFromRole && memberSet.has(leaderFromRole)) {
    return leaderFromRole;
  }
  return normalizedMemberIds[0] ?? "";
}

export function TeamPage(props: TeamPageProps) {
  const [error, setError] = useState<string | null>(null);
  const [warning, setWarning] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [teamsSidebarCollapsed, setTeamsSidebarCollapsed] = useState(false);
  const [teamDebugTag, setTeamDebugTag] = useState<TeamDebugTag>("run_ops");
  useEffect(() => {
    document.body.classList.add("teams-page");
    return () => {
      document.body.classList.remove("teams-page");
    };
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
  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(null);

  const [teamCreateState, dispatchTeamCreate] = useReducer(
    reduceTeamCreateState,
    undefined,
    createInitialTeamCreateState
  );
  const createDraftPersistErrorRef = useRef<string | null>(null);
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
  const [memberDiscoveryCardsById, setMemberDiscoveryCardsById] = useState<
    Record<string, AgentDiscoveryCardRecord | null>
  >({});
  const [memberDiscoveryCardLoadingById, setMemberDiscoveryCardLoadingById] = useState<
    Record<string, boolean>
  >({});
  const [mainTasks, setMainTasks] = useState<TeamMainTaskRecord[]>([]);
  const [mainTasksLoading, setMainTasksLoading] = useState(false);
  const [selectedMainTaskId, setSelectedMainTaskId] = useState("");
  const [mainTaskMessages, setMainTaskMessages] = useState<TeamConversationMessageRecord[]>([]);
  const [mainTaskMessagesLoading, setMainTaskMessagesLoading] = useState(false);
  const [mainTaskMessageDraft, setMainTaskMessageDraft] = useState("");
  const [compilePreviewContextId, setCompilePreviewContextId] = useState("");
  const [compiledRunPreview, setCompiledRunPreview] =
    useState<TeamMainTaskRunCompilePreviewRecord | null>(null);

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
  const selectedTeamMemberIds = useMemo(() => {
    if (!selectedTeam) {
      return [];
    }
    return parseTeamSpecMembers(selectedTeam.spec).map((member) => member.member_id);
  }, [selectedTeam]);
  const selectedTeamLeaderMemberId = useMemo(() => {
    if (!selectedTeam) {
      return "";
    }
    return resolveTeamLeaderMemberId(selectedTeam.spec, selectedTeamMemberIds);
  }, [selectedTeam, selectedTeamMemberIds]);
  useEffect(() => {
    setCompiledRunPreview(null);
    setCompilePreviewContextId("");
    setMainTasks([]);
    setMainTasksLoading(false);
    setSelectedMainTaskId("");
    setMainTaskMessages([]);
    setMainTaskMessagesLoading(false);
    setMainTaskMessageDraft("");
  }, [selectedTeamId]);
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
        resolveTeamMemberAgentStatuses(team.spec, agents, teamMemberAgentsById)
      );
    }
    return next;
  }, [agents, teamMemberAgentsById, teams]);
  const teamMemberSummaryByTeamId = useMemo(() => {
    const next = new Map<string, TeamMemberAgentStatusSummary>();
    for (const team of teams) {
      const members = teamMemberStatusByTeamId.get(team.id) ?? [];
      next.set(team.id, summarizeTeamMemberAgentStatuses(members));
    }
    return next;
  }, [teamMemberStatusByTeamId, teams]);
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
  const teamForgeAgents = useMemo(
    () => selectTeamForgeAgents(agents, teamForgeAgentIds),
    [agents, teamForgeAgentIds]
  );
  const leaderAgent = useMemo(
    () => teamForgeAgents.find((agent) => agent.id === leaderMemberId) ?? null,
    [teamForgeAgents, leaderMemberId]
  );
  const hasForgeAgents = teamForgeAgents.length > 0;
  const forgeRoleTag = useMemo<TeamForgeRoleTag | null>(() => {
    if (createTeamStage === 1) {
      return "leader";
    }
    if (createTeamStage === 2) {
      return "worker";
    }
    return null;
  }, [createTeamStage]);
  const canForgeAgentsInStage = !useSpecOverride && forgeRoleTag !== null;

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
      if (useSpecOverride && target !== 0 && target !== 3) {
        return false;
      }
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
    [createStageReadiness, createTeamStage, useSpecOverride]
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
    onCreateRun,
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
    selectedMemberSnapshot,
    activeRunIdRef,
    eventsRef,
    memberEventsRef,
    setBusy,
    setError,
    setAgents,
    setTeams,
    setSelectedTeamId,
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

  useTeamCreateModalLifecycleEffects({
    token: props.token,
    busy,
    showCreateTeamModal,
    leaderMemberId,
    teamForgeAgents,
    parseError: parseErrorMessage,
    setError,
    setForgeDefaultWorktreeRoot,
    setLeaderMemberId,
    setShowCreateTeamModal,
    setCreateTeamStage,
  });

  const openCreateTeamModal = useCallback(
    (mode: TeamCreateEntryMode) => {
      const isManualSpec = mode === "manual_spec";
      const { draft: restoredDraft, error: restoreError } = loadTeamCreateDraft(mode);
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
      } else {
        setCreateTeamStage(0);
        setUseSpecOverride(isManualSpec);
        setShowCreateTeamModal(true);
        setShowForgeAgentForm(false);
        setForgeAgentWorktreeError(null);
      }
      void refreshAgents().catch((err) => {
        setError(parseErrorMessage(err));
      });
    },
    [
      patchTeamCreate,
      refreshAgents,
      resetTeamDraft,
      setWarning,
      setCreateTeamStage,
      setShowCreateTeamModal,
      setShowForgeAgentForm,
      setForgeAgentWorktreeError,
      setUseSpecOverride,
    ]
  );

  const openCreateTeamWizardModal = useCallback(() => {
    openCreateTeamModal("wizard");
  }, [openCreateTeamModal]);

  const openCreateTeamManualModal = useCallback(() => {
    openCreateTeamModal("manual_spec");
  }, [openCreateTeamModal]);

  const closeCreateTeamModal = () => {
    setShowCreateTeamModal(false);
    setCreateTeamStage(0);
    setShowForgeAgentForm(false);
    setForgeAgentWorktreeError(null);
  };

  const openForgeAgentForm = () => {
    if (!forgeRoleTag) {
      setError("Open Agent Forge in Leader Forge or Recruit Workers stage.");
      return;
    }
    setError(null);
    setForgeAgentWorktreeError(null);
    setShowForgeAgentForm(true);
    const teamToken = newTeamName.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-");
    const prefix = teamToken || "team";
    const defaultName =
      forgeRoleTag === "leader"
        ? `${prefix}-leader`
        : forgeRoleTag === "worker"
          ? `${prefix}-worker-${Math.max(1, workers.length + 1)}`
          : `${prefix}-agent-${Math.max(1, agents.length + 1)}`;
    setForgeAgentName(defaultName);
    setForgeAgentWorktreeMode("use_existing");
    if (forgeRoleTag === "leader") {
      const normalizedRoot =
        normalizeWorkdirInput(forgeDefaultWorktreeRoot) || DEFAULT_WORKTREE_ROOT;
      setForgeAgentWorkdir(buildLeaderForgeDefaultWorkdir(normalizedRoot, defaultName));
    } else {
      setForgeAgentWorkdir((prev) =>
        resolveWorkdirForModalOpen(
          prev,
          "use_existing",
          forgeDefaultWorktreeRoot,
          DEFAULT_WORKTREE_ROOT
        )
      );
    }
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
    if (!forgeRoleTag) {
      setError("Role tag is unavailable in this stage. Switch to Leader or Worker stage.");
      return;
    }
    const isLeaderForge = forgeRoleTag === "leader";
    const effectiveWorktreeMode = isLeaderForge
      ? "use_existing"
      : forgeAgentWorktreeMode;
    const effectiveWorktreeRepo = isLeaderForge ? "" : forgeAgentWorktreeRepo.trim();
    const effectiveWorktreeRef = isLeaderForge ? "" : forgeAgentWorktreeRef.trim();
    const name = forgeAgentName.trim() || "agent";
    const normalizedRoot =
      normalizeWorkdirInput(forgeDefaultWorktreeRoot) || DEFAULT_WORKTREE_ROOT;
    const workdirInput = normalizeWorkdirInput(forgeAgentWorkdir);
    const workdir =
      isLeaderForge && !workdirInput
        ? buildLeaderForgeDefaultWorkdir(normalizedRoot, name)
        : workdirInput;
    const workdirPayload =
      effectiveWorktreeMode === "create_worktree" &&
      normalizedRoot &&
      workdir === normalizedRoot
        ? ""
        : workdir;
    if (!workdirPayload && effectiveWorktreeMode !== "create_worktree") {
      setError("Forge agent workdir is required");
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
        source: "team_forge",
        worktree_mode: effectiveWorktreeMode,
        worktree_repo: effectiveWorktreeRepo || null,
        worktree_ref: effectiveWorktreeRef || null,
        code_mode: forgeAgentCodeMode,
      });
      setAgents((prev) => [created, ...prev.filter((agent) => agent.id !== created.id)]);
      setTeamForgeAgentIds((prev) =>
        prev.includes(created.id) ? prev : [...prev, created.id]
      );
      if (forgeRoleTag === "leader") {
        setLeaderMemberId(created.id);
      } else if (forgeRoleTag === "worker") {
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
    setCreateTeamStage((prev) => {
      if (useSpecOverride && prev === 3) {
        return 0;
      }
      return clampCreateTeamStage(prev - 1);
    });
  };

  const onCreateTeam = async () => {
    const name = newTeamName.trim();
    if (!name) {
      setError("Team name is required");
      return;
    }
    if (!useSpecOverride && !leaderMemberId.trim()) {
      setError("Leader member is required");
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
      setError("Leader/member assignments must be unique");
      return;
    }
    setBusy("create-team");
    setError(null);
    setWarning(null);
    try {
      const specPayload = useSpecOverride
        ? parseRequiredJson(newTeamSpec, "Team spec")
        : builtTeamSpec;
      const created = await api.createTeam(props.token, {
        name,
        description: newTeamDescription.trim() || undefined,
        spec: specPayload,
      });
      const staleForgeAgentIds = resolveUnusedTeamForgeAgentIds(
        teamForgeAgentIds,
        created.spec
      );
      const { deletedForgeAgentIds, cleanupErrors } = await cleanupUnusedTeamForgeAgents(
        props.token,
        staleForgeAgentIds,
        api.deleteAgent
      );
      if (deletedForgeAgentIds.length > 0) {
        const deletedSet = new Set(deletedForgeAgentIds);
        setAgents((prev) => prev.filter((agent) => !deletedSet.has(agent.id)));
      }
      setTeams((prev) => [...prev, created].sort((a, b) => a.name.localeCompare(b.name)));
      setSelectedTeamId(created.id);
      clearTeamCreateDraft();
      resetTeamDraft();
      closeCreateTeamModal();
      const cleanupWarning = buildTeamForgeCleanupWarning(cleanupErrors);
      if (cleanupWarning) {
        setWarning(cleanupWarning);
      }
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

  const selectedConversation = useMemo(
    () => mainTasks.find((task) => task.id === selectedMainTaskId) ?? null,
    [mainTasks, selectedMainTaskId]
  );

  const refreshMainTasks = useCallback(
    async (teamId: string) => {
      setMainTasksLoading(true);
      try {
        const list = await api.listTeamMainTasks(props.token, teamId, 100);
        const sorted = sortMainTasksByActivity(list);
        setMainTasks(sorted);
        setSelectedMainTaskId((prev) => {
          const selectedId = prev.trim();
          const hasSelected =
            selectedId.length > 0 && sorted.some((task) => task.id === selectedId);
          const nextSelectedId = hasSelected ? selectedId : sorted[0]?.id ?? "";
          return nextSelectedId;
        });
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
        setMainTasksLoading(false);
      }
    },
    [props.token]
  );

  const refreshMainTaskMessages = useCallback(
    async (mainTaskIdOverride?: string) => {
      const teamId = selectedTeamId;
      const mainTaskId = (mainTaskIdOverride ?? selectedMainTaskId).trim();
      if (!teamId || !mainTaskId) {
        setMainTaskMessages([]);
        return;
      }
      setMainTaskMessagesLoading(true);
      try {
        const messages = await api.listTeamMainTaskMessages(props.token, teamId, mainTaskId, {
          limit: 200,
        });
        setMainTaskMessages(messages);
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
        setMainTaskMessagesLoading(false);
      }
    },
    [props.token, selectedMainTaskId, selectedTeamId]
  );

  useEffect(() => {
    if (!selectedTeamId) {
      return;
    }
    void refreshMainTasks(selectedTeamId);
  }, [refreshMainTasks, selectedTeamId]);

  useEffect(() => {
    if (!selectedTeamId) {
      return;
    }
    const mainTaskId = selectedMainTaskId.trim();
    if (!mainTaskId) {
      setMainTaskMessages([]);
      return;
    }
    const matchesSelectedTeam = mainTasks.some(
      (task) => task.id === mainTaskId && task.team_id === selectedTeamId
    );
    if (!matchesSelectedTeam) {
      setMainTaskMessages([]);
      return;
    }
    void refreshMainTaskMessages(mainTaskId);
  }, [mainTasks, refreshMainTaskMessages, selectedMainTaskId, selectedTeamId]);

  const ensureConversationForMessage = useCallback(
    async (messageText: string): Promise<string> => {
      if (!selectedTeamId) {
        throw new Error("Select a team first");
      }
      const selectedId = selectedMainTaskId.trim();
      const selectedExists = mainTasks.some(
        (task) => task.id === selectedId && task.team_id === selectedTeamId
      );
      if (selectedId && selectedExists) {
        return selectedId;
      }
      const sorted = sortMainTasksByActivity(mainTasks);
      if (sorted.length > 0) {
        const latestId = sorted[0].id;
        setSelectedMainTaskId(latestId);
        return latestId;
      }
      const created = await api.createTeamMainTask(props.token, selectedTeamId, {
        title: buildAutoConversationTitle(messageText),
        created_by_actor_id: "user",
        conversation_mode: "group_chat",
        context: {
          source: "team_workbench",
          auto_created: true,
        },
      });
      setMainTasks((prev) =>
        sortMainTasksByActivity([created.task, ...prev.filter((task) => task.id !== created.task.id)])
      );
      setSelectedMainTaskId(created.task.id);
      setMainTaskMessages([]);
      return created.task.id;
    },
    [mainTasks, props.token, selectedMainTaskId, selectedTeamId]
  );

  const onSendMainTaskMessage = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    const text = mainTaskMessageDraft.trim();
    if (!text) {
      setError("Conversation message is required");
      return;
    }
    setBusy("send-main-task-message");
    setError(null);
    setWarning(null);
    try {
      const mainTaskId = await ensureConversationForMessage(text);
      const mentionActorIds = extractMentionedActorIds(text, selectedTeamMemberIds);
      const routePlan = resolveMainTaskMailboxRoutePlan(
        selectedTeamMemberIds,
        mentionActorIds,
        selectedTeamLeaderMemberId
      );
      const chatPayload = buildMailboxChatPayload(text, {
        mention_actor_ids: mentionActorIds,
      });
      const message = await api.sendTeamMainTaskMessage(props.token, selectedTeamId, mainTaskId, {
        from_actor_id: "user",
        route: "group_chat",
        payload: chatPayload,
      });
      setMainTaskMessages((prev) =>
        [...prev, message].sort((left, right) => left.message_id - right.message_id)
      );
      if (activeRunIdForSelectedTeam) {
        const toActorIds = routePlan.toActorIds;
        if (routePlan.fromActorId && toActorIds.length > 0) {
          const forwardedPayload = {
            ...chatPayload,
            main_task_id: mainTaskId,
            main_task_message_id: message.message_id,
            main_task_conversation_id: message.conversation_id,
            human_actor_id: "user",
            delivery_scope: mentionActorIds.length > 0 ? "mention" : "broadcast",
          };
          await Promise.all(
            toActorIds.map((toActorId) =>
              api.sendTeamRunMessage(props.token, activeRunIdForSelectedTeam, {
                from_actor_id: routePlan.fromActorId,
                to_actor_id: toActorId,
                channel: "default",
                transport: "local",
                payload: forwardedPayload,
                idempotency_key: `main-task:${mainTaskId}:${message.message_id}:${toActorId}`,
              })
            )
          );
          await Promise.all([
            refreshSnapshot(activeRunIdForSelectedTeam),
            refreshEvents(activeRunIdForSelectedTeam),
          ]);
        }
      } else {
        setWarning(
          "Conversation message saved. No active run in this team, so agents were not notified."
        );
      }
      setMainTaskMessageDraft("");
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    ensureConversationForMessage,
    activeRunIdForSelectedTeam,
    mainTaskMessageDraft,
    props.token,
    refreshEvents,
    refreshSnapshot,
    selectedTeamLeaderMemberId,
    selectedTeamMemberIds,
    selectedTeamId,
    setWarning,
  ]);

  const onRefreshMemberConsole = useCallback(async () => {
    if (selectedMemberSnapshot) {
      await loadMemberEvents("replace");
      return;
    }
    if (activeRunIdForSelectedTeam) {
      await refreshEvents(activeRunIdForSelectedTeam);
    }
  }, [activeRunIdForSelectedTeam, loadMemberEvents, refreshEvents, selectedMemberSnapshot]);

  const onLoadOlderMemberConsole = useCallback(async () => {
    if (!selectedMemberSnapshot) {
      return;
    }
    await loadMemberEvents("prepend");
  }, [loadMemberEvents, selectedMemberSnapshot]);

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
    setTab("mailbox");
  }, [setSelectedMemberId, setTab]);

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

  const onUpdateWorker = (
    index: number,
    field: "member_id" | "description" | "model" | "prompt" | "custom_skills",
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

  const panelPrimaryButtonClassName = TEAM_PANEL_PRIMARY_BUTTON_CLASS;
  const panelSecondaryButtonClassName = TEAM_PANEL_SECONDARY_BUTTON_CLASS;
  const panelInputClassName = `${TEAM_PANEL_INPUT_CLASS} shadow-sm`;
  const runInputValidation = useMemo(() => validateRunInputJson(runInput), [runInput]);
  const runInputHasError = runInputValidation.error !== null;
  const canCreateRun = busy !== "create-run" && !runInputHasError;
  const canCompileMainTask = busy !== "compile-main-task" && selectedConversation !== null;
  const panelGhostButtonClassName = TEAM_PANEL_GHOST_BUTTON_CLASS;
  const teamSectionCardClassName =
    "min-h-0 min-w-0 rounded-2xl border border-ui-border bg-ui-surface p-4 shadow-sm";
  const teamSectionCardLargeClassName =
    "min-h-0 rounded-2xl border border-ui-border bg-ui-surface p-5 shadow-sm";
  const teamSectionHeadingClassName = "text-sm font-semibold text-ui-text-primary";
  const teamSectionTitleClassName = "text-base font-semibold text-ui-text-primary";
  const teamSectionBodyTextClassName = "mt-2 text-sm text-ui-text-muted";
  const teamSectionHintTextClassName = "mt-2 text-xs text-ui-text-muted";
  const teamDebugTabsClassName =
    "flex flex-wrap items-center gap-2 rounded-lg border border-ui-border bg-ui-surface-soft p-1";
  const teamDebugTabBaseClassName =
    "rounded-md px-3 py-1.5 text-xs font-medium transition sm:text-sm";
  const teamDebugTabActiveClassName =
    `${teamDebugTabBaseClassName} bg-brand-primary text-ui-text-inverse shadow-sm`;
  const teamDebugTabIdleClassName =
    `${teamDebugTabBaseClassName} text-ui-text-muted hover:bg-ui-surface hover:text-ui-text-primary`;
  const teamCreateModalHeaderClassName =
    "modal-head flex flex-wrap items-start justify-between gap-3 border-b border-ui-border pb-3";
  const teamCreateModalTitleClassName =
    "text-lg font-semibold tracking-tight text-ui-text-primary";
  const teamCreateStageClassName =
    "team-create-stage flex min-h-[64px] flex-col items-start gap-1 rounded-xl border px-3 py-2 text-left transition";
  const teamCreateStageActiveClassName =
    `${teamCreateStageClassName} active border-brand-primary bg-brand-primary text-ui-text-inverse shadow-sm`;
  const teamCreateStageCompletedClassName =
    `${teamCreateStageClassName} completed border-[color:var(--status-active-border)] bg-[color:var(--status-active-bg)] text-[color:var(--status-active-ink)]`;
  const teamCreateStageLockedClassName =
    `${teamCreateStageClassName} locked cursor-not-allowed border-ui-border bg-ui-surface-muted text-ui-text-muted`;
  const teamCreateStageIdleClassName =
    `${teamCreateStageClassName} border-ui-border-strong bg-ui-surface text-ui-text-secondary hover:border-ui-border-emphasis hover:bg-ui-surface-soft`;
  const teamCreateCheckItemReadyClassName =
    "team-create-check-item ready flex items-center gap-2 rounded-lg border border-[color:var(--status-active-border)] bg-[color:var(--status-active-bg)] px-3 py-2 text-sm text-[color:var(--status-active-ink)]";
  const teamCreateCheckItemPendingClassName =
    "team-create-check-item pending flex items-center gap-2 rounded-lg border border-ui-border bg-ui-surface-soft px-3 py-2 text-sm text-ui-text-muted";
  const teamCreateAgentEntryClassName =
    "team-create-agent-entry rounded-xl border border-ui-border bg-ui-surface-soft/70 p-4";
  const teamCreateForgeMetaClassName =
    "team-create-forge-agent-meta mono mt-2 grid grid-cols-2 gap-2 rounded-lg border border-ui-border bg-ui-surface px-3 py-2 text-xs text-ui-text-muted";
  const teamCreateLaunchMetaClassName =
    "mono mt-3 grid min-w-0 gap-2 text-xs text-ui-text-secondary sm:grid-cols-3";
  const teamCreateLaunchMetaItemClassName =
    "rounded-lg border border-ui-border bg-ui-surface px-3 py-2";
  const modalFieldClassName =
    "w-full rounded-lg border border-ui-border-strong bg-ui-surface px-3 py-2 text-sm text-ui-text-primary shadow-sm outline-none transition focus:border-ui-border-emphasis focus:ring-2 focus:ring-ui-border disabled:cursor-not-allowed disabled:bg-ui-surface-muted disabled:text-ui-text-muted";
  const modalMonoFieldClassName = `${modalFieldClassName} font-mono text-xs leading-5`;
  const teamRunMetaItemClassName =
    "rounded-lg border border-ui-border bg-ui-surface-soft px-3 py-2";
  const tabNeedsActiveRun = tabRequiresActiveRun(tab);
  const showRunContextLoading = tab !== "runs" && tabNeedsActiveRun && runsLoading && !activeRunForSelectedTeam;
  const showNoActiveRunNotice = tab !== "runs" && tabNeedsActiveRun && !runsLoading && !activeRunForSelectedTeam;
  const onRefreshActiveRun = useCallback(() => {
    if (!activeRunIdForSelectedTeam) return;
    void refreshRun(activeRunIdForSelectedTeam).catch((err) => setError(parseErrorMessage(err)));
  }, [activeRunIdForSelectedTeam, refreshRun, setError]);
  const onRefreshMainTasks = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    setError(null);
    await refreshMainTasks(selectedTeamId);
  }, [refreshMainTasks, selectedTeamId]);
  const onCompileMainTaskRunPreview = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    const mainTaskId = selectedMainTaskId.trim();
    if (!mainTaskId) {
      setError("Select a conversation first");
      return;
    }
    setBusy("compile-main-task");
    setError(null);
    try {
      const preview = await api.compileTeamMainTaskRunPreview(
        props.token,
        selectedTeamId,
        mainTaskId,
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
    selectedMainTaskId,
    selectedTeamId,
    setBusy,
    setError,
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
    selectedTeamId,
    setBusy,
    setError,
  ]);
  const conversationPanel = (
    <div className="space-y-3">
      <TeamMainTaskPanel
        tasks={mainTasks}
        tasksLoading={mainTasksLoading}
        selectedMainTaskId={selectedMainTaskId}
        onSelectedMainTaskIdChange={setSelectedMainTaskId}
        onRefreshTasks={onRefreshMainTasks}
        messageDraft={mainTaskMessageDraft}
        onMessageDraftChange={setMainTaskMessageDraft}
        onSendMessage={onSendMainTaskMessage}
        onRefreshMessages={refreshMainTaskMessages}
        messages={mainTaskMessages}
        memberLiveStates={selectedTeamMemberLiveStates}
        messagesLoading={mainTaskMessagesLoading}
        busy={busy}
        formatTs={formatTs}
        toPrettyJson={toPrettyJson}
      />
    </div>
  );

  const compilePreviewPanel = (
    <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
      <h4 className={teamSectionHeadingClassName}>Compile Conversation</h4>
      <p className={teamSectionBodyTextClassName}>
        Internal debug entry for compiling conversation into deterministic run payload preview.
      </p>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <button
          className={panelPrimaryButtonClassName}
          onClick={onCompileMainTaskRunPreview}
          disabled={!canCompileMainTask}
        >
          Compile Preview
        </button>
        <span className="mono text-xs text-ui-text-muted">
          {selectedConversation
            ? `selected_conversation=${selectedConversation.title} [${selectedConversation.status}]`
            : "selected_conversation=-"}
        </span>
      </div>
      <input
        className={`${panelInputClassName} mt-2`}
        placeholder="context_id override (optional)"
        value={compilePreviewContextId}
        onChange={(event) => setCompilePreviewContextId(event.target.value)}
      />
      {compiledRunPreview ? (
        <div className="mt-3 space-y-2 rounded-lg border border-ui-border bg-ui-surface-soft p-3">
          <div className="mono text-xs text-ui-text-secondary">
            <div>
              <strong>conversation_id:</strong> {compiledRunPreview.conversation_id}
            </div>
            <div>
              <strong>context_id:</strong> {compiledRunPreview.run_payload.context_id}
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              className={panelSecondaryButtonClassName}
              onClick={onUseCompiledRunPayload}
            >
              Use Payload in Create Run
            </button>
            <button
              type="button"
              className={panelPrimaryButtonClassName}
              onClick={onCreateRunFromCompiledPreview}
              disabled={busy === "create-run"}
            >
              Create Run from Preview
            </button>
          </div>
          <pre className="teams-step-body mono max-h-72 overflow-auto rounded-lg border border-ui-border bg-ui-surface px-3 py-2 text-xs text-ui-text-secondary">
            {toPrettyJson({
              conversation_id: compiledRunPreview.conversation_id,
              run_payload: compiledRunPreview.run_payload,
            })}
          </pre>
        </div>
      ) : (
        <p className={teamSectionHintTextClassName}>
          Select a conversation to preview compiled run payload.
        </p>
      )}
    </div>
  );

  const runOpsPanel = (
    <div className="space-y-3">
      <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
        <h4 className={teamSectionHeadingClassName}>Create Run</h4>
        <p className={teamSectionBodyTextClassName}>
          Debug entry for manually starting a Team run.
        </p>
        <div className="form-row mt-3">
          <input
            className={panelInputClassName}
            placeholder="context_id (optional, auto-generated when empty)"
            value={runContextId}
            onChange={(event) => setRunContextId(event.target.value)}
          />
          <button
            className={panelPrimaryButtonClassName}
            onClick={onCreateRun}
            disabled={!canCreateRun}
            title={runInputValidation.error ?? "Create run"}
          >
            Create Run
          </button>
        </div>
        <p className={teamSectionHintTextClassName}>
          <code>context_id</code> can be empty. Use one when you want retries/resume grouped
          under the same context.
        </p>
        <textarea
          className={`${TEAM_PANEL_TEXTAREA_CLASS} mt-3`}
          rows={8}
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
        />
        {runInputValidation.error ? (
          <p className="mt-2 text-xs text-rose-600" role="alert">
            {runInputValidation.error}
          </p>
        ) : (
          <p className={teamSectionHintTextClassName}>
            Accepts any valid JSON value. Shortcut: Ctrl/Cmd + Enter to create run.
          </p>
        )}
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <button
            type="button"
            className={panelSecondaryButtonClassName}
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
          </button>
          <button
            type="button"
            className={panelSecondaryButtonClassName}
            onClick={() => setRunInput("{}")}
          >
            Set Empty Object
          </button>
          <button
            type="button"
            className={panelSecondaryButtonClassName}
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
          </button>
          <button
            type="button"
            className={panelSecondaryButtonClassName}
            onClick={() => setRunInput("")}
            disabled={runInput.trim().length === 0}
          >
            Clear
          </button>
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
        <div className="form-row mt-3">
          <input
            className={panelInputClassName}
            placeholder="existing run_id"
            value={runLookupId}
            onChange={(event) => setRunLookupId(event.target.value)}
          />
          <button
            className={panelSecondaryButtonClassName}
            onClick={onLoadRunById}
            disabled={busy === "load-run"}
          >
            Load Run
          </button>
        </div>
      </div>
      {compilePreviewPanel}
    </div>
  );

  return (
    <div className="mx-auto flex h-[var(--agenthub-vh,100vh)] w-full max-w-[1600px] flex-col gap-5 overflow-y-auto overscroll-y-contain px-3 py-3 sm:px-4 lg:px-6 [&>*]:shrink-0">
      <header>
        <div className="flex min-w-0 items-center gap-2">
          <button
            className="icon-button output-agents-toggle"
            onClick={() => setTeamsSidebarCollapsed((previous) => !previous)}
            title={teamsSidebarCollapsed ? "Show teams panel" : "Hide teams panel"}
            aria-label={teamsSidebarCollapsed ? "Show teams panel" : "Hide teams panel"}
          >
            <i
              className={teamsSidebarCollapsed ? "bi bi-chevron-right" : "bi bi-chevron-left"}
              aria-hidden="true"
            />
          </button>
          <h1>AgentHub Teams</h1>
        </div>
        <div className="session team-session">
          <span
            className="session-connection muted"
            title="Teams console"
            role="status"
            aria-live="polite"
          >
            <span className="session-connection-dot" aria-hidden="true" />
            <span>Teams</span>
          </span>
          <span>{props.auth.username}</span>
          <a className="icon-button" href="/" title="Agents" aria-label="Agents">
            <i className="bi bi-arrow-left" aria-hidden="true" />
          </a>
          {props.auth.role === "root" && (
            <a className="icon-button" href="/admin" title="Admin" aria-label="Admin">
              <i className="bi bi-gear" aria-hidden="true" />
            </a>
          )}
          <button
            className="rounded-[var(--radius-sm)] border-0 bg-[var(--ink)] px-[14px] py-2 font-semibold tracking-[0.01em] text-white transition hover:opacity-90"
            onClick={props.onLogout}
          >
            Logout
          </button>
        </div>
      </header>

      {error && <ErrorBanner message={error} onClose={() => setError(null)} />}
      {warning && (
        <div className="team-create-warning rounded-xl" role="status">
          <p className="text-sm text-amber-900">{warning}</p>
          <button
            type="button"
            className={panelSecondaryButtonClassName}
            onClick={() => setWarning(null)}
            aria-label="Dismiss warning"
          >
            Dismiss
          </button>
        </div>
      )}

      <div
        className={
          teamsSidebarCollapsed
            ? "teams-layout grid min-h-0 flex-1 gap-5 lg:grid-cols-[minmax(0,1fr)]"
            : "teams-layout grid min-h-0 flex-1 gap-5 lg:grid-cols-[minmax(320px,380px)_minmax(0,1fr)]"
        }
      >
        {!teamsSidebarCollapsed && (
          <TeamSidebar
            busy={busy}
            onRefreshTeams={refreshTeams}
            onOpenCreateTeamWizard={openCreateTeamWizardModal}
            onOpenCreateTeamManual={openCreateTeamManualModal}
            draftTeamName={newTeamName}
            leaderMemberId={leaderMemberId}
            configuredWorkerCount={configuredWorkerCount}
            teams={teams}
            selectedTeamId={selectedTeamId}
            teamMemberSummaryByTeamId={teamMemberSummaryByTeamId}
            onSelectTeam={(teamId) => {
              if (teamId !== selectedTeamId) {
                setActiveRunId(null);
              }
              setSelectedTeamId(teamId);
              setRunLookupId("");
            }}
          />
        )}

        <div className="teams-main flex min-h-0 min-w-0 flex-col gap-5 overflow-y-auto pb-2 pr-1 [&>*]:shrink-0">
          {!selectedTeam && (
            <div className={teamSectionCardLargeClassName}>
              <h2 className="text-lg font-semibold tracking-tight text-ui-text-primary">
                Team Workbench
              </h2>
              <p className={teamSectionBodyTextClassName}>
                Select a team from the left panel to start team conversations and supervise execution.
              </p>
            </div>
          )}

          {selectedTeam && (
            <>
              <TeamMemberStatusStrip members={selectedTeamMemberLiveStates} />
              <TeamTabsBar tab={tab} onTabChange={setTab} />

              {tab === "runs" && (
                <TeamRunPanel
                  selectedTeam={selectedTeam}
                  busy={busy}
                  onDeleteTeam={onDeleteTeam}
                  onStartTeam={onCreateRun}
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

              {tab !== "runs" && activeRunForSelectedTeam && !runsLoading && (
                <TeamActiveRunPanel
                  run={activeRunForSelectedTeam}
                  busy={busy}
                  canResumeRun={canResumeActiveRun}
                  canRestartRun={canRestartActiveRun}
                  onRefresh={onRefreshActiveRun}
                  onCancel={onCancelRun}
                  onResume={onResumeRun}
                  onRestart={onRestartRun}
                  formatTs={formatTs}
                  cardClassName={teamSectionCardClassName}
                  titleClassName={teamSectionTitleClassName}
                  metaItemClassName={teamRunMetaItemClassName}
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
                            Conversation is available before execution starts.
                          </p>
                        </div>
                      )}
                      {conversationPanel}
                    </>
                  )}

                  {tab === "agent_acp" && activeRunForSelectedTeam && (
                    <TeamMemberAcpPanel
                      snapshot={snapshot}
                      selectedMemberId={selectedMemberId}
                      onSelectedMemberIdChange={setSelectedMemberId}
                      selectedMemberSnapshot={selectedMemberSnapshot}
                      memberEvents={memberEvents}
                      memberEventsHasMore={memberEventsHasMore}
                      memberEventsLoading={memberEventsLoading}
                      eventsLoading={eventsLoading}
                      oldestMemberEventId={oldestMemberEventId}
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

                  {tab === "mailbox" && activeRunForSelectedTeam && (
                    <TeamMailboxPanel
                      mode="full"
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

                  {tab === "member_console" && activeRunForSelectedTeam && (
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
                      memberDiscoveryCard={selectedMemberDiscoveryCard}
                      memberDiscoveryCardLoading={selectedMemberDiscoveryCardLoading}
                      onRefresh={onRefreshMemberConsole}
                      onLoadOlder={onLoadOlderMemberConsole}
                      toPrettyJson={toPrettyJson}
                      formatTs={formatTs}
                    />
                  )}

                  {tab === "debug" && (
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
                          mode="advanced_only"
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
                    </>
                  )}
                </div>
              )}
            </>
          )}
        </div>
      </div>

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
            className={TEAM_CREATE_MODAL_CARD_CLASS}
            role="dialog"
            aria-modal="true"
            aria-labelledby="team-create-title"
          >
            <div className={teamCreateModalHeaderClassName}>
              <h3 id="team-create-title" className={teamCreateModalTitleClassName}>
                Team Forge
              </h3>
              <div className="team-create-head-meta flex flex-wrap items-center gap-2">
                <span className={TEAM_CREATE_STAGE_BADGE_CLASS}>
                  Stage {createTeamStage + 1}/{CREATE_TEAM_STAGE_TITLES.length}
                </span>
                <span className={TEAM_CREATE_STAGE_BADGE_CLASS}>
                  {useSpecOverride ? "Manual Spec" : "Guided Wizard"}
                </span>
              </div>
            </div>

            <div className="team-create-progress mt-4 grid gap-2 md:grid-cols-4">
              {CREATE_TEAM_STAGE_TITLES.map((title, index) => {
                const stageIndex = index as CreateTeamStage;
                const isActive = stageIndex === createTeamStage;
                const isCompleted = stageIndex < createTeamStage;
                const isManualSkipped =
                  useSpecOverride && stageIndex !== 0 && stageIndex !== 3;
                const isLocked = isManualSkipped || !canEnterCreateStage(stageIndex);
                return (
                  <button
                    key={title}
                    className={`${
                      isActive
                        ? teamCreateStageActiveClassName
                        : isCompleted
                          ? teamCreateStageCompletedClassName
                          : isLocked
                            ? teamCreateStageLockedClassName
                          : teamCreateStageIdleClassName
                    }`}
                    onClick={() => onSelectCreateTeamStage(stageIndex)}
                    type="button"
                    aria-disabled={isLocked && !isActive && !isCompleted}
                    title={
                      isLocked && !isActive && !isCompleted
                        ? isManualSkipped
                          ? "Manual spec mode skips this stage"
                          : "Complete previous stage requirements first"
                        : undefined
                    }
                    >
                    <span className="team-create-stage-index text-[11px] font-medium uppercase tracking-wide opacity-80">
                      #{index + 1}
                    </span>
                    <span className="team-create-stage-title text-sm font-semibold">{title}</span>
                  </button>
                );
              })}
            </div>

            <div className="modal-body mt-4 space-y-4">
              <div className="team-create-checklist grid gap-2 sm:grid-cols-2">
                {questChecklist.map((item) => (
                  <div
                    key={item.key}
                    className={`${
                      item.ready
                        ? teamCreateCheckItemReadyClassName
                        : teamCreateCheckItemPendingClassName
                    }`}
                  >
                    <span
                      className="team-create-check-icon inline-flex h-5 w-5 items-center justify-center rounded-full border border-current text-[11px]"
                      aria-hidden="true"
                    >
                      {item.ready ? "✓" : "○"}
                    </span>
                    <span>{item.label}</span>
                  </div>
                ))}
              </div>

              {createTeamStage !== 0 && !useSpecOverride && (
                <div className={teamCreateAgentEntryClassName}>
                  <div className="team-create-agent-entry-head flex flex-wrap items-center justify-between gap-2">
                    <h4 className={teamSectionTitleClassName}>Agent Forge</h4>
                    <button
                      className={panelSecondaryButtonClassName}
                      onClick={showForgeAgentForm ? closeForgeAgentForm : openForgeAgentForm}
                      disabled={!canForgeAgentsInStage || forgeAgentBusy}
                      type="button"
                    >
                      {showForgeAgentForm ? "Hide" : "New Agent"}
                    </button>
                  </div>
                  <p className={teamSectionBodyTextClassName}>
                    Role tag follows the current stage. In Team, worker is a role assigned to a
                    forged agent.
                  </p>
                  <div className={teamCreateForgeMetaClassName}>
                    <span>role_tag</span>
                    <span className="text-ui-text-primary">{forgeRoleTag ?? "-"}</span>
                  </div>
                  {!canForgeAgentsInStage && (
                    <div className={TEAM_CREATE_NOTE_WARNING_CLASS}>
                      Agent Forge is available only in Leader Forge or Recruit Workers stage.
                    </div>
                  )}
                  {showForgeAgentForm && (
                    <div className={TEAM_CREATE_NOTE_INFO_CLASS}>
                      Agent create modal is open. Submit to create and auto-assign by role tag.
                    </div>
                  )}
                </div>
              )}

              {createTeamStage === 0 && (
                <div className={TEAM_CREATE_PANEL_CARD_CLASS}>
                  <h4 className={teamSectionTitleClassName}>Mission Brief</h4>
                  <div className="team-create-mission-intro mt-2">
                    <p className="text-sm text-ui-text-muted">
                      Pick a team name and description first. This is the party identity shown in
                      the workbench.
                    </p>
                  </div>
                  <p className={TEAM_CREATE_NOTE_INFO_CLASS}>
                    {useSpecOverride
                      ? "Manual Spec entry selected. Next stage jumps directly to Launch Team."
                      : "Guided Wizard entry selected. Continue to Leader Forge next."}
                  </p>
                  {!isMissionBriefReady && (
                    <p className={TEAM_CREATE_NOTE_WARNING_CLASS}>
                      Team name is required before entering the next stage.
                    </p>
                  )}
                  <input
                    className={`${modalFieldClassName} mt-3`}
                    placeholder="team name"
                    value={newTeamName}
                    onChange={(event) => setNewTeamName(event.target.value)}
                  />
                  <input
                    className={`${modalFieldClassName} mt-3`}
                    placeholder="description (optional)"
                    value={newTeamDescription}
                    onChange={(event) => setNewTeamDescription(event.target.value)}
                  />
                </div>
              )}

              {createTeamStage === 1 && (
                <div className={TEAM_CREATE_PANEL_CARD_CLASS}>
                  <h4 className={teamSectionTitleClassName}>Leader Forge</h4>
                  <p className={teamSectionBodyTextClassName}>
                    Choose the leader from member agents created in this Team Forge session only.
                  </p>
                  {!isLeaderForgeReady && hasForgeAgents && (
                    <p className={TEAM_CREATE_NOTE_WARNING_CLASS}>
                      Select one forged leader agent to continue.
                    </p>
                  )}
                  {!hasForgeAgents && (
                    <p className={teamSectionBodyTextClassName}>
                      No forged agents yet. Create one in the Agent Forge entry above.
                    </p>
                  )}
                  <select
                    className={`${modalFieldClassName} mt-3`}
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
                  <div className={TEAM_CREATE_STEP_PREVIEW_CLASS}>
                    <div>agent_id: {leaderMemberId || "-"}</div>
                    <div>workdir: {leaderAgent?.workdir ?? "-"}</div>
                  </div>
                  <select
                    className={`${modalFieldClassName} mt-3`}
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
                  <div className="team-skill-tags mt-3 flex flex-wrap gap-2">
                    {TEAM_SKILL_OPTIONS.map((skill) => {
                      const selected = leaderSkills.includes(skill);
                      const isRequired = REQUIRED_TEAM_LEADER_SKILLS.includes(skill);
                      return (
                        <button
                          key={`leader-skill-${skill}`}
                          type="button"
                          className={
                            selected
                              ? TEAM_CREATE_SKILL_TAG_SELECTED_CLASS
                              : TEAM_CREATE_SKILL_TAG_IDLE_CLASS
                          }
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
                    className={`${modalFieldClassName} mt-3`}
                    placeholder="leader custom skills (comma separated, optional)"
                    value={leaderCustomSkills}
                    onChange={(event) => setLeaderCustomSkills(event.target.value)}
                    disabled={useSpecOverride}
                  />
                  <textarea
                    className={`${modalMonoFieldClassName} mt-3`}
                    rows={4}
                    placeholder="leader prompt"
                    value={leaderPrompt}
                    onChange={(event) => setLeaderPrompt(event.target.value)}
                    disabled={useSpecOverride}
                  />
                </div>
              )}

              {createTeamStage === 2 && (
                <div className={TEAM_CREATE_PANEL_CARD_CLASS}>
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <h4 className={teamSectionTitleClassName}>Recruit Workers</h4>
                    <div className="flex flex-wrap gap-2">
                      <button
                        className={panelSecondaryButtonClassName}
                        onClick={onAddWorker}
                        disabled={useSpecOverride || !hasForgeAgents}
                      >
                        Add Worker
                      </button>
                      <button
                        className={panelSecondaryButtonClassName}
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
                  <p className={teamSectionBodyTextClassName}>
                    Build your party from Team Forge member agents only. Worker is a role
                    assignment for those agents, and model/prompt/skills can still be customized at
                    team level.
                  </p>
                  {unassignedWorkerSlots > 0 && (
                    <p className={TEAM_CREATE_NOTE_WARNING_CLASS}>
                      {unassignedWorkerSlots} worker slot
                      {unassignedWorkerSlots > 1 ? "s are" : " is"} currently unassigned and will
                      be ignored unless selected.
                    </p>
                  )}
                  <div className="team-create-worker-grid mt-3 grid gap-3 lg:grid-cols-2">
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
                        <div
                          key={`worker-${index}`}
                          className={TEAM_CREATE_WORKER_CARD_CLASS}
                        >
                          <div className="team-create-worker-head flex items-center justify-between gap-2">
                            <strong>Worker {index + 1}</strong>
                            <button
                              className={panelGhostButtonClassName}
                              onClick={() => onRemoveWorker(index)}
                              disabled={useSpecOverride}
                              type="button"
                            >
                              Remove
                            </button>
                          </div>
                          <select
                            className={`${modalFieldClassName} mt-3`}
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
                          <div className={TEAM_CREATE_STEP_PREVIEW_MUTED_CLASS}>
                            <div>agent_id: {worker.member_id || "-"}</div>
                            <div>workdir: {workerAgent?.workdir ?? "-"}</div>
                          </div>
                          <input
                            className={`${modalFieldClassName} mt-3`}
                            placeholder="worker description (identity card)"
                            value={worker.description}
                            onChange={(event) =>
                              onUpdateWorker(index, "description", event.target.value)
                            }
                            disabled={useSpecOverride}
                          />
                          <select
                            className={`${modalFieldClassName} mt-3`}
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
                          <div className="team-skill-tags mt-3 flex flex-wrap gap-2">
                            {TEAM_SKILL_OPTIONS.map((skill) => {
                              const selected = worker.skills.includes(skill);
                              const isRequired = REQUIRED_TEAM_WORKER_SKILLS.includes(skill);
                              return (
                                <button
                                  key={`worker-skill-${index}-${skill}`}
                                  type="button"
                                  className={
                                    selected
                                      ? TEAM_CREATE_SKILL_TAG_SELECTED_CLASS
                                      : TEAM_CREATE_SKILL_TAG_IDLE_CLASS
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
                            className={`${modalFieldClassName} mt-3`}
                            placeholder="worker custom skills (comma separated, optional)"
                            value={worker.custom_skills}
                            onChange={(event) =>
                              onUpdateWorker(index, "custom_skills", event.target.value)
                            }
                            disabled={useSpecOverride}
                          />
                          <textarea
                            className={`${modalMonoFieldClassName} mt-3`}
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
                    <p className="mt-3 text-sm text-ui-text-muted">
                      No workers configured. Team will run with leader only.
                    </p>
                  )}
                  {hasDuplicateMembers && (
                    <div className="team-create-warning mt-3 rounded-xl border border-rose-200 bg-rose-50 p-3">
                      <p className="muted text-sm text-rose-700">
                        Duplicate assignments detected: {duplicateMemberIds.join(", ")}. Leader
                        and member assignments must reference different agents.
                      </p>
                      <button
                        className={`${panelSecondaryButtonClassName} mt-2`}
                        onClick={onResolveDuplicateWorkers}
                        type="button"
                      >
                        Resolve Duplicates
                      </button>
                    </div>
                  )}
                </div>
              )}

              {createTeamStage === 3 && (
                <div className={TEAM_CREATE_PANEL_CARD_CLASS}>
                  <h4 className={teamSectionTitleClassName}>Launch Team</h4>
                  <p className={teamSectionBodyTextClassName}>
                    Final review before deployment.
                  </p>
                  <div className={teamCreateLaunchMetaClassName}>
                    <span className={teamCreateLaunchMetaItemClassName}>
                      team={newTeamName.trim() || "-"}
                    </span>
                    <span className={teamCreateLaunchMetaItemClassName}>
                      leader={leaderMemberId.trim() || "-"}
                    </span>
                    <span className={teamCreateLaunchMetaItemClassName}>
                      workers={configuredWorkerCount}
                    </span>
                  </div>
                  {useSpecOverride ? (
                    <p className="mt-3 text-sm text-ui-text-muted">
                      Manual Spec entry: edit full team spec JSON directly.
                    </p>
                  ) : (
                    <p className="mt-3 text-sm text-ui-text-muted">
                      Guided wizard generated this spec:
                      `leader_plan` → `worker_*` → `leader_synthesize`.
                    </p>
                  )}
                  <textarea
                    className={`${modalMonoFieldClassName} mt-3`}
                    rows={12}
                    value={displayedTeamSpec}
                    onChange={(event) => setNewTeamSpec(event.target.value)}
                    readOnly={!useSpecOverride}
                  />
                </div>
              )}
            </div>

            <div className={TEAM_CREATE_ACTIONS_BAR_CLASS}>
              {!canAdvanceCreateStage && currentStageBlockReason && (
                <span className="team-create-actions-note mr-auto text-sm text-amber-700">
                  {currentStageBlockReason}
                </span>
              )}
              <button
                className={panelGhostButtonClassName}
                onClick={closeCreateTeamModal}
                disabled={busy === "create-team"}
                type="button"
              >
                Cancel
              </button>
              <button
                className={panelGhostButtonClassName}
                onClick={goToPrevCreateTeamStage}
                disabled={createTeamStage === 0 || busy === "create-team"}
                type="button"
              >
                Back
              </button>
              {createTeamStage < 3 && (
                <button
                  className={panelPrimaryButtonClassName}
                  onClick={goToNextCreateTeamStage}
                  disabled={!canAdvanceCreateStage || busy === "create-team"}
                  type="button"
                >
                  Next Stage
                </button>
              )}
              {createTeamStage === 3 && (
                <button
                  className={panelPrimaryButtonClassName}
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
              showWorktreeAdvancedOptions={forgeRoleTag !== "leader"}
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
