import React, { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  AgentRecord,
  AgentEvent,
  api,
  TeamActorMessageRecord,
  TeamDefinitionRecord,
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
import {
  buildTeamSpecFromForm,
  clampCreateTeamStage,
  formatTeamForgeWorktreeError,
  parseErrorMessage,
  parseOptionalInteger,
  parseOptionalJson,
  parseRequiredJson,
  resolveTeamModelOptions,
} from "./team/create_helpers";
import {
  MailboxTemplateKey,
  buildMailboxChatPayload,
  buildMailboxConversationKey,
  buildMailboxPayloadTemplate,
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
  buildDefaultWorkerDraft,
  buildTeamMemberLiveStates,
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
  upsertAgentEventList,
  upsertEventList,
  upsertRun,
} from "./team/page_helpers";
import {
  mergeRunPages,
  mergeTeamRunList,
  resolveRunStatusFilter,
  selectTeamPreviewEvents,
  type TeamRunStatusFilter,
} from "./team/run_helpers";
import {
  CREATE_TEAM_STAGE_TITLES,
  DEFAULT_TEAM_CONTROL_STATE,
  DEFAULT_TEAM_MAILBOX_STATE,
  DEFAULT_TEAM_RUN_BROWSER_STATE,
  DEFAULT_TEAM_UI_STATE,
  DEFAULT_WORKTREE_ROOT,
  EVENT_PAGE_LIMIT,
  MAILBOX_TEMPLATE_OPTIONS,
  MEMBER_EVENT_PAGE_LIMIT,
  TEAM_RUN_STATUS_FILTER_OPTIONS,
  TEAM_RUN_PAGE_LIMIT,
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
  TEAM_PANEL_GHOST_BUTTON_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_TAB_BAR_CLASS,
  TEAM_TAB_BUTTON_ACTIVE_CLASS,
  TEAM_TAB_BUTTON_IDLE_CLASS,
} from "../ui/tailwind_classes";

export {
  buildMailboxChatPayload,
  buildMailboxConversationKey,
  buildMailboxPayloadTemplate,
  countUnreadConversationMessages,
  mergeMailboxMessages,
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
type TeamCreateEntryMode = "wizard" | "manual_spec";
type TeamDebugTag = "run_ops" | "step_ops" | "mailbox_raw";

const TEAM_EVENT_PREVIEW_LIMIT = 5;
export function TeamPage(props: TeamPageProps) {
  const [error, setError] = useState<string | null>(null);
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
  const teamSpecMemberIds = useMemo(() => {
    const ids = new Set<string>();
    for (const team of teams) {
      for (const member of parseTeamSpecMembers(team.spec)) {
        ids.add(member.member_id);
      }
    }
    return [...ids];
  }, [teams]);
  useEffect(() => {
    const listedAgentIds = new Set(agents.map((agent) => agent.id));
    const unresolvedMemberIds = teamSpecMemberIds.filter(
      (memberId) =>
        !listedAgentIds.has(memberId) && !(memberId in teamMemberAgentsById)
    );
    if (unresolvedMemberIds.length === 0) {
      return;
    }

    let canceled = false;
    const loadMissingMemberAgents = async () => {
      const resolved = await Promise.all(
        unresolvedMemberIds.map(async (memberId) => {
          try {
            const agent = await api.getAgent(props.token, memberId);
            return [memberId, agent] as const;
          } catch {
            return [memberId, null] as const;
          }
        })
      );
      if (canceled) {
        return;
      }
      setTeamMemberAgentsById((prev) => {
        const next = { ...prev };
        for (const [memberId, agent] of resolved) {
          next[memberId] = agent;
        }
        return next;
      });
    };

    void loadMissingMemberAgents();
    return () => {
      canceled = true;
    };
  }, [agents, props.token, teamMemberAgentsById, teamSpecMemberIds]);
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
          return mergeRunPages(otherTeamRuns, merged);
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
    if (!activeRunIdForSelectedTeam) return;
    const actorId = (actorIdOverride ?? inboxActorId).trim();
    if (!actorId) {
      throw new Error("Inbox actor_id is required");
    }
    const limit = parseOptionalInteger(inboxLimit, "Inbox limit") ?? 100;
    const afterId = parseOptionalInteger(inboxAfterId, "Inbox after_id");
    const list = await api.listTeamRunInbox(props.token, activeRunIdForSelectedTeam, {
      actor_id: actorId,
      limit,
      after_id: afterId,
      include_delivered: inboxIncludeDelivered,
    });
    setInbox(list);
  }, [
    activeRunIdForSelectedTeam,
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
    if (!activeRunIdForSelectedTeam) {
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
        const run = await refreshRun(activeRunIdForSelectedTeam);
        if (canceled) return;
        if (selectedTeamId && run.team_id !== selectedTeamId) {
          setError(
            `Run ${run.id} belongs to team ${run.team_id}. Select that team to view it.`
          );
          setActiveRunId((current) => (current === run.id ? null : current));
          return;
        }
        await Promise.all([
          refreshSteps(activeRunIdForSelectedTeam),
          refreshEvents(activeRunIdForSelectedTeam),
          refreshSnapshot(activeRunIdForSelectedTeam),
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
    activeRunIdForSelectedTeam,
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
    if (!activeRunIdForSelectedTeam || !eventsAutoRefresh) return;
    const timer = window.setInterval(() => {
      if (tab === "mailbox") {
        void refreshSnapshot(activeRunIdForSelectedTeam).catch(() => undefined);
        const actorId = chatActors.inboxActorId.trim();
        if (actorId) {
          void loadInbox(actorId).catch(() => undefined);
        }
        return;
      }
      void refreshRun(activeRunIdForSelectedTeam).catch(() => undefined);
      void refreshEvents(activeRunIdForSelectedTeam).catch(() => undefined);
      void refreshSnapshot(activeRunIdForSelectedTeam).catch(() => undefined);
    }, 4000);
    return () => {
      window.clearInterval(timer);
    };
  }, [
    activeRunIdForSelectedTeam,
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
    if (!activeRunIdForSelectedTeam || !actorId) {
      setInbox([]);
      return;
    }
    setInboxActorId(actorId);
    void loadInbox(actorId).catch((err) => {
      setError(parseErrorMessage(err));
    });
  }, [activeRunIdForSelectedTeam, chatActors.inboxActorId, loadInbox, setInbox, setInboxActorId]);

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

  const openCreateTeamModal = useCallback(
    (mode: TeamCreateEntryMode) => {
      const isManualSpec = mode === "manual_spec";
      setError(null);
      setCreateTeamStage(0);
      resetTeamDraft();
      setUseSpecOverride(isManualSpec);
      setShowCreateTeamModal(true);
      setShowForgeAgentForm(false);
      setForgeAgentWorktreeError(null);
      void refreshAgents().catch((err) => {
        setError(parseErrorMessage(err));
      });
    },
    [
      refreshAgents,
      resetTeamDraft,
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
    if (!forgeRoleTag) {
      setError("Role tag is unavailable in this stage. Switch to Leader or Worker stage.");
      return;
    }
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
        source: "team_forge",
        worktree_mode: forgeAgentWorktreeMode,
        worktree_repo: forgeAgentWorktreeRepo.trim() || null,
        worktree_ref: forgeAgentWorktreeRef.trim() || null,
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
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    const runId = runLookupId.trim();
    if (!runId) {
      setError("Run ID is required");
      return;
    }
    setBusy("load-run");
    setError(null);
    try {
      const run = await refreshRun(runId);
      if (run.team_id !== selectedTeamId) {
        setError(
          `Run ${run.id} belongs to team ${run.team_id}. Load Run only applies to the selected team.`
        );
        return;
      }
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
    if (!activeRunForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    const runId = activeRunForSelectedTeam.id;
    setBusy("cancel-run");
    setError(null);
    try {
      const canceled = await api.cancelTeamRun(props.token, runId);
      if (selectedTeamId && canceled.team_id !== selectedTeamId) {
        setError(
          `Run ${canceled.id} belongs to team ${canceled.team_id}. Cancel applies only to the selected team.`
        );
        return;
      }
      setRuns((prev) => upsertRun(prev, canceled));
      await Promise.all([refreshEvents(runId), refreshSnapshot(runId)]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onResumeRun = async () => {
    if (!activeRunForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    const runId = activeRunForSelectedTeam.id;
    setBusy("resume-run");
    setError(null);
    try {
      const resumed = await api.resumeTeamRun(props.token, runId);
      if (selectedTeamId && resumed.team_id !== selectedTeamId) {
        setError(
          `Run ${resumed.id} belongs to team ${resumed.team_id}. Resume applies only to the selected team.`
        );
        return;
      }
      setRuns((prev) => upsertRun(prev, resumed));
      setActiveRunId(resumed.id);
      setRunLookupId(resumed.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onRestartRun = async () => {
    if (!activeRunForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    const runId = activeRunForSelectedTeam.id;
    setBusy("restart-run");
    setError(null);
    try {
      const restarted = await api.restartTeamRun(props.token, runId);
      if (selectedTeamId && restarted.team_id !== selectedTeamId) {
        setError(
          `Run ${restarted.id} belongs to team ${restarted.team_id}. Restart applies only to the selected team.`
        );
        return;
      }
      setRuns((prev) => upsertRun(prev, restarted));
      setActiveRunId(restarted.id);
      setRunLookupId(restarted.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onSubmitStep = async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
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
      const created = await api.submitTeamRunStep(props.token, activeRunIdForSelectedTeam, {
        step_key: stepKey.trim(),
        member_id: stepMemberId.trim(),
        depends_on: parseCsvList(stepDependsOn),
        input: parseOptionalJson(stepInput, "Step input"),
      });
      await Promise.all([
        refreshRun(activeRunIdForSelectedTeam),
        refreshSteps(activeRunIdForSelectedTeam),
        refreshEvents(activeRunIdForSelectedTeam),
        refreshSnapshot(activeRunIdForSelectedTeam),
      ]);
      setSelectedStepId(created.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onApplyStepAction = async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
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
        await api.startTeamRunStep(props.token, activeRunIdForSelectedTeam, selectedStepId, {
          remote_task_id: stepRemoteTaskId.trim() || undefined,
        });
      } else if (stepAction === "complete") {
        await api.completeTeamRunStep(props.token, activeRunIdForSelectedTeam, selectedStepId, {
          output: parseOptionalJson(stepOutput, "Step output"),
        });
      } else if (stepAction === "fail") {
        const errorText = stepFailText.trim();
        if (!errorText) {
          throw new Error("Fail reason is required");
        }
        await api.failTeamRunStep(props.token, activeRunIdForSelectedTeam, selectedStepId, {
          error_text: errorText,
        });
      } else if (stepAction === "input_required") {
        await api.setTeamRunStepInputRequired(props.token, activeRunIdForSelectedTeam, selectedStepId, {
          reason: stepInputReason.trim() || undefined,
          input: parseOptionalJson(stepInputRequiredPayload, "Input required payload"),
        });
      } else {
        await api.resumeTeamRunStep(props.token, activeRunIdForSelectedTeam, selectedStepId, {
          input: parseOptionalJson(stepResumePayload, "Resume payload"),
        });
      }
      await Promise.all([
        refreshRun(activeRunIdForSelectedTeam),
        refreshSteps(activeRunIdForSelectedTeam),
        refreshEvents(activeRunIdForSelectedTeam),
        refreshSnapshot(activeRunIdForSelectedTeam),
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
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
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
      await api.sendTeamRunMessage(props.token, activeRunIdForSelectedTeam, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: "default",
        transport: "local",
        payload: buildMailboxChatPayload(text),
      });
      setChatDraft("");
      await refreshSnapshot(activeRunIdForSelectedTeam);
      await loadInbox(toActorId);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onSendMessage = async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
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
      await api.sendTeamRunMessage(props.token, activeRunIdForSelectedTeam, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: msgChannel.trim() || undefined,
        transport: msgTransport,
        route: parseOptionalJson(msgRoute, "Message route"),
        payload: parseRequiredJson(msgPayload, "Message payload"),
        idempotency_key: msgIdempotencyKey.trim() || undefined,
      });
      if (tab === "mailbox") {
        await refreshSnapshot(activeRunIdForSelectedTeam);
        if (inboxActorId.trim()) {
          await loadInbox();
        }
      } else {
        await Promise.all([
          refreshEvents(activeRunIdForSelectedTeam),
          refreshSnapshot(activeRunIdForSelectedTeam),
        ]);
      }
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onRefreshInbox = async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
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
    if (!activeRunIdForSelectedTeam) return;
    const actorId = inboxActorId.trim() || message.to_actor_id;
    setBusy(`ack-${message.message_id}`);
    setError(null);
    try {
      await api.ackTeamRunMessage(
        props.token,
        activeRunIdForSelectedTeam,
        message.message_id,
        actorId
      );
      if (tab === "mailbox") {
        await Promise.all([loadInbox(actorId), refreshSnapshot(activeRunIdForSelectedTeam)]);
      } else {
        await Promise.all([
          loadInbox(),
          refreshEvents(activeRunIdForSelectedTeam),
          refreshSnapshot(activeRunIdForSelectedTeam),
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

  const panelPrimaryButtonClassName = TEAM_PANEL_PRIMARY_BUTTON_CLASS;
  const panelSecondaryButtonClassName = TEAM_PANEL_SECONDARY_BUTTON_CLASS;
  const panelInputClassName =
    `${TEAM_PANEL_INPUT_CLASS} shadow-sm focus:border-slate-400 focus:ring-slate-300`;
  const panelRefreshButtonClassName = TEAM_PANEL_REFRESH_BUTTON_CLASS;
  const panelGhostButtonClassName = TEAM_PANEL_GHOST_BUTTON_CLASS;
  const modalFieldClassName =
    "w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 shadow-sm outline-none transition focus:border-slate-400 focus:ring-2 focus:ring-slate-300 disabled:cursor-not-allowed disabled:bg-slate-100 disabled:text-slate-500";
  const modalMonoFieldClassName = `${modalFieldClassName} font-mono text-xs leading-5`;

  return (
    <div className="mx-auto flex h-[var(--agenthub-vh,100vh)] min-h-[var(--agenthub-vh,100vh)] w-full max-w-[1600px] flex-col gap-5 overflow-y-auto overscroll-y-contain px-3 py-3 sm:px-4 lg:px-6 [&>*]:shrink-0">
      <header className="mb-0 flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <button
            className="icon-button small output-agents-toggle"
            onClick={() => setTeamsSidebarCollapsed((previous) => !previous)}
            title={teamsSidebarCollapsed ? "Show teams panel" : "Hide teams panel"}
            aria-label={teamsSidebarCollapsed ? "Show teams panel" : "Hide teams panel"}
          >
            <i
              className={teamsSidebarCollapsed ? "bi bi-chevron-right" : "bi bi-chevron-left"}
              aria-hidden="true"
            />
          </button>
          <h1 className="whitespace-normal text-xl font-semibold tracking-tight text-slate-900 sm:text-2xl">
            AgentHub Teams
          </h1>
        </div>
        <div className="team-session flex max-w-full flex-wrap items-center justify-end gap-2 rounded-xl border border-slate-200 bg-white px-2 py-1.5 shadow-sm">
          <a className="icon-button" href="/" title="Back" aria-label="Back">
            <i className="bi bi-arrow-left" aria-hidden="true" />
          </a>
          <span className="max-w-[44vw] truncate text-sm font-medium text-slate-700 sm:max-w-none">
            {props.auth.username}
          </span>
          <button className={panelSecondaryButtonClassName} onClick={props.onLogout}>
            Logout
          </button>
        </div>
      </header>

      {error && <ErrorBanner message={error} onClose={() => setError(null)} />}

      <div
        className={
          teamsSidebarCollapsed
            ? "teams-layout grid min-h-0 gap-5 lg:grid-cols-[minmax(0,1fr)]"
            : "teams-layout grid min-h-0 gap-5 lg:grid-cols-[minmax(320px,380px)_minmax(0,1fr)]"
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
              setSelectedTeamId(teamId);
              setRunLookupId("");
            }}
          />
        )}

        <div className="teams-main flex min-h-0 min-w-0 flex-col gap-5 [&>*]:shrink-0">
          {!selectedTeam && (
            <div className="min-h-0 rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
              <h2 className="text-lg font-semibold text-slate-900">Team Workbench</h2>
              <p className="mt-2 text-sm text-slate-600">
                Select a team from the left panel to manage runs, steps, and messages.
              </p>
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

              {runsLoading && !activeRunForSelectedTeam && (
                <div className="min-h-0 min-w-0 rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
                  <p className="text-sm text-slate-600">Loading run context for selected team...</p>
                </div>
              )}

              {activeRunForSelectedTeam && !runsLoading && (
                <>
                  <div className="min-h-0 min-w-0 rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
                    <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
                      <h3 className="text-base font-semibold text-slate-900">Active Run</h3>
                      <div className="flex flex-wrap items-center gap-2">
                        <button
                          className={panelRefreshButtonClassName}
                          title="Refresh active run"
                          aria-label="Refresh active run"
                          onClick={() => {
                            if (!activeRunIdForSelectedTeam) return;
                            void refreshRun(activeRunIdForSelectedTeam).catch((err) =>
                              setError(parseErrorMessage(err))
                            );
                          }}
                        >
                          <i className="bi bi-arrow-clockwise" aria-hidden="true" />
                          <span>Refresh</span>
                        </button>
                        <button
                          className={panelSecondaryButtonClassName}
                          onClick={onCancelRun}
                          disabled={
                            busy === "cancel-run" ||
                            activeRunForSelectedTeam.status === "canceled"
                          }
                        >
                          Cancel Run
                        </button>
                        <button
                          className={panelSecondaryButtonClassName}
                          onClick={onResumeRun}
                          disabled={busy === "resume-run" || !canResumeActiveRun}
                          title={
                            canResumeActiveRun
                              ? "Resume a failed/canceled run"
                              : "Resume is available for failed/canceled runs"
                          }
                        >
                          Resume Run
                        </button>
                        <button
                          className={panelSecondaryButtonClassName}
                          onClick={onRestartRun}
                          disabled={busy === "restart-run" || !canRestartActiveRun}
                          title={
                            canRestartActiveRun
                              ? "Create a fresh run from the same context/input"
                              : "Restart is available for completed/failed/canceled runs"
                          }
                        >
                          Restart Run
                        </button>
                      </div>
                    </div>
                    <div className="mt-3 grid min-w-0 gap-2 text-sm text-slate-700 sm:grid-cols-2 xl:grid-cols-3">
                      <span className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
                        <strong>ID:</strong> <code>{activeRunForSelectedTeam.id}</code>
                      </span>
                      <span className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
                        <strong>Status:</strong>{" "}
                        <StatusBadge
                          label={activeRunForSelectedTeam.status}
                          tone={resolveTeamRunStatusTone(activeRunForSelectedTeam.status)}
                          className="team-status"
                          title={`run status: ${activeRunForSelectedTeam.status}`}
                        />
                      </span>
                      <span className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
                        <strong>Context:</strong> {activeRunForSelectedTeam.context_id}
                      </span>
                      <span className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
                        <strong>Created:</strong> {formatTs(activeRunForSelectedTeam.created_at)}
                      </span>
                      <span className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
                        <strong>Started:</strong> {formatTs(activeRunForSelectedTeam.started_at)}
                      </span>
                      <span className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
                        <strong>Ended:</strong> {formatTs(activeRunForSelectedTeam.ended_at)}
                      </span>
                    </div>
                  </div>

                  <div className={`mt-2 ${TEAM_TAB_BAR_CLASS}`}>
                    <button
                      className={tab === "overview" ? TEAM_TAB_BUTTON_ACTIVE_CLASS : TEAM_TAB_BUTTON_IDLE_CLASS}
                      onClick={() => setTab("overview")}
                    >
                      Overview
                    </button>
                    <button
                      className={tab === "events" ? TEAM_TAB_BUTTON_ACTIVE_CLASS : TEAM_TAB_BUTTON_IDLE_CLASS}
                      onClick={() => setTab("events")}
                    >
                      Events
                    </button>
                    <button
                      className={tab === "steps" ? TEAM_TAB_BUTTON_ACTIVE_CLASS : TEAM_TAB_BUTTON_IDLE_CLASS}
                      onClick={() => setTab("steps")}
                    >
                      Steps
                    </button>
                    <button
                      className={tab === "mailbox" ? TEAM_TAB_BUTTON_ACTIVE_CLASS : TEAM_TAB_BUTTON_IDLE_CLASS}
                      onClick={() => setTab("mailbox")}
                    >
                      Mailbox
                    </button>
                    <button
                      className={
                        tab === "member_console"
                          ? TEAM_TAB_BUTTON_ACTIVE_CLASS
                          : TEAM_TAB_BUTTON_IDLE_CLASS
                      }
                      onClick={() => setTab("member_console")}
                    >
                      Member Console
                    </button>
                    <button
                      className={tab === "debug" ? TEAM_TAB_BUTTON_ACTIVE_CLASS : TEAM_TAB_BUTTON_IDLE_CLASS}
                      onClick={() => setTab("debug")}
                    >
                      Debug
                    </button>
                  </div>

                  <div className="flex min-w-0 flex-col gap-3">
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

                    {tab === "mailbox" && (
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

                    {tab === "debug" && (
                      <>
                        <div className="rounded-xl border border-slate-200 bg-white p-3 shadow-sm">
                          <div className="flex flex-wrap items-center justify-between gap-3">
                            <h3 className="text-sm font-semibold text-slate-900">Debug Tools</h3>
                            <div className="flex flex-wrap items-center gap-2 rounded-lg border border-slate-200 bg-slate-50 p-1">
                              <button
                                className={`rounded-md px-3 py-1.5 text-xs font-medium transition sm:text-sm ${
                                  teamDebugTag === "run_ops"
                                    ? "bg-slate-900 text-white shadow-sm"
                                    : "text-slate-600 hover:bg-slate-50 hover:text-slate-900"
                                }`}
                                onClick={() => setTeamDebugTag("run_ops")}
                              >
                                Run Ops
                              </button>
                              <button
                                className={`rounded-md px-3 py-1.5 text-xs font-medium transition sm:text-sm ${
                                  teamDebugTag === "step_ops"
                                    ? "bg-slate-900 text-white shadow-sm"
                                    : "text-slate-600 hover:bg-slate-50 hover:text-slate-900"
                                }`}
                                onClick={() => setTeamDebugTag("step_ops")}
                              >
                                Step Ops
                              </button>
                              <button
                                className={`rounded-md px-3 py-1.5 text-xs font-medium transition sm:text-sm ${
                                  teamDebugTag === "mailbox_raw"
                                    ? "bg-slate-900 text-white shadow-sm"
                                    : "text-slate-600 hover:bg-slate-50 hover:text-slate-900"
                                }`}
                                onClick={() => setTeamDebugTag("mailbox_raw")}
                              >
                                Mailbox Raw
                              </button>
                            </div>
                          </div>
                        </div>

                        {teamDebugTag === "run_ops" && (
                          <div className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
                            <h4 className="text-sm font-semibold text-slate-900">Load Existing Run</h4>
                            <p className="muted mt-2 text-sm text-slate-600">
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
                        )}

                        {teamDebugTag === "step_ops" && (
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

                        {teamDebugTag === "mailbox_raw" && (
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
                </>
              )}
            </>
          )}
        </div>
      </div>

      {showCreateTeamModal && (
        <div
          className="modal-backdrop team-create-modal-backdrop fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-slate-950/40 px-3 py-6 sm:py-10"
          role="presentation"
          onClick={(event) => {
            if (event.target === event.currentTarget && busy !== "create-team") {
              closeCreateTeamModal();
            }
          }}
        >
          <div
            className="modal team-create-modal w-full max-w-5xl rounded-2xl border border-slate-200 bg-white p-4 shadow-2xl sm:p-5"
            role="dialog"
            aria-modal="true"
            aria-labelledby="team-create-title"
          >
            <div className="modal-head flex flex-wrap items-start justify-between gap-3 border-b border-slate-200 pb-3">
              <h3 id="team-create-title" className="text-lg font-semibold tracking-tight text-slate-900">
                Team Forge
              </h3>
              <div className="team-create-head-meta flex flex-wrap items-center gap-2">
                <span className="badge rounded-full border border-slate-300 bg-slate-100 px-2.5 py-1 text-xs font-medium text-slate-700">
                  Stage {createTeamStage + 1}/{CREATE_TEAM_STAGE_TITLES.length}
                </span>
                <span className="badge rounded-full border border-slate-300 bg-slate-100 px-2.5 py-1 text-xs font-medium text-slate-700">
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
                    className={`team-create-stage flex min-h-[64px] flex-col items-start gap-1 rounded-xl border px-3 py-2 text-left transition ${
                      isActive
                        ? "active border-slate-900 bg-slate-900 text-white shadow-sm"
                        : isCompleted
                          ? "completed border-emerald-300 bg-emerald-50 text-emerald-700"
                          : isLocked
                            ? "locked cursor-not-allowed border-slate-200 bg-slate-100 text-slate-400"
                          : "border-slate-300 bg-white text-slate-700 hover:border-slate-400 hover:bg-slate-50"
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
                    className={`team-create-check-item flex items-center gap-2 rounded-lg border px-3 py-2 text-sm ${
                      item.ready
                        ? "ready border-emerald-300 bg-emerald-50 text-emerald-700"
                        : "pending border-slate-200 bg-slate-50 text-slate-600"
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

              {createTeamStage !== 0 && (
                <div className="team-create-agent-entry rounded-xl border border-slate-200 bg-slate-50/70 p-4">
                  <div className="team-create-agent-entry-head flex flex-wrap items-center justify-between gap-2">
                    <h4 className="text-base font-semibold text-slate-900">Agent Forge</h4>
                    <button
                      className={panelSecondaryButtonClassName}
                      onClick={showForgeAgentForm ? closeForgeAgentForm : openForgeAgentForm}
                      disabled={!canForgeAgentsInStage || forgeAgentBusy}
                      type="button"
                    >
                      {showForgeAgentForm ? "Hide" : "New Agent"}
                    </button>
                  </div>
                  <p className="muted mt-2 text-sm text-slate-600">
                    Role tag follows the current stage. Leader stage creates leader agents, worker
                    stage creates worker agents.
                  </p>
                  <div className="team-create-forge-agent-meta mono mt-2 grid grid-cols-2 gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs text-slate-600">
                    <span>role_tag</span>
                    <span className="text-slate-900">{forgeRoleTag ?? "-"}</span>
                  </div>
                  {!canForgeAgentsInStage && (
                    <div className="team-create-stage-note mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700">
                      Agent Forge is available only in Leader Forge or Recruit Workers stage.
                    </div>
                  )}
                  {showForgeAgentForm && (
                    <div className="team-create-stage-note mt-2 rounded-lg border border-sky-200 bg-sky-50 px-3 py-2 text-sm text-sky-700">
                      Agent create modal is open. Submit to create and auto-assign by role tag.
                    </div>
                  )}
                </div>
              )}

              {createTeamStage === 0 && (
                <div className="team-create-panel rounded-xl border border-slate-200 bg-slate-50/70 p-4">
                  <h4 className="text-base font-semibold text-slate-900">Mission Brief</h4>
                  <div className="team-create-mission-intro mt-2">
                    <p className="muted text-sm text-slate-600">
                      Pick a team name and description first. This is the party identity shown in
                      the workbench.
                    </p>
                  </div>
                  <p className="team-create-stage-note mt-2 rounded-lg border border-sky-200 bg-sky-50 px-3 py-2 text-sm text-sky-700">
                    {useSpecOverride
                      ? "Manual Spec entry selected. Next stage jumps directly to Launch Team."
                      : "Guided Wizard entry selected. Continue to Leader Forge next."}
                  </p>
                  {!isMissionBriefReady && (
                    <p className="team-create-stage-note mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700">
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
                <div className="team-create-panel rounded-xl border border-slate-200 bg-slate-50/70 p-4">
                  <h4 className="text-base font-semibold text-slate-900">Leader Forge</h4>
                  <p className="muted mt-2 text-sm text-slate-600">
                    Choose the leader from agents created in this Team Forge session only.
                  </p>
                  {!isLeaderForgeReady && hasForgeAgents && (
                    <p className="team-create-stage-note mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700">
                      Select one forged leader agent to continue.
                    </p>
                  )}
                  {!hasForgeAgents && (
                    <p className="muted mt-2 text-sm text-slate-600">
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
                  <div className="teams-step-body mono mt-3 rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs text-slate-600">
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
                              ? "team-skill-tag selected rounded-full border border-slate-900 bg-slate-900 px-3 py-1 text-xs font-medium text-white transition"
                              : "team-skill-tag rounded-full border border-slate-300 bg-white px-3 py-1 text-xs font-medium text-slate-700 transition hover:border-slate-400 hover:bg-slate-50"
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
                <div className="team-create-panel rounded-xl border border-slate-200 bg-slate-50/70 p-4">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <h4 className="text-base font-semibold text-slate-900">Recruit Workers</h4>
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
                  <p className="muted mt-2 text-sm text-slate-600">
                    Build your party from Team Forge agents only. Worker model/prompt/skills can
                    still be customized at team level.
                  </p>
                  {unassignedWorkerSlots > 0 && (
                    <p className="team-create-stage-note mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700">
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
                          className="teams-worker-card rounded-xl border border-slate-200 bg-white p-3 shadow-sm"
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
                          <div className="teams-step-body mono mt-3 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-xs text-slate-600">
                            <div>agent_id: {worker.member_id || "-"}</div>
                            <div>workdir: {workerAgent?.workdir ?? "-"}</div>
                          </div>
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
                                      ? "team-skill-tag selected rounded-full border border-slate-900 bg-slate-900 px-3 py-1 text-xs font-medium text-white transition"
                                      : "team-skill-tag rounded-full border border-slate-300 bg-white px-3 py-1 text-xs font-medium text-slate-700 transition hover:border-slate-400 hover:bg-slate-50"
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
                    <p className="muted mt-3 text-sm text-slate-600">
                      No workers configured. Team will run with leader only.
                    </p>
                  )}
                  {hasDuplicateMembers && (
                    <div className="team-create-warning mt-3 rounded-xl border border-rose-200 bg-rose-50 p-3">
                      <p className="muted text-sm text-rose-700">
                        Duplicate assignments detected: {duplicateMemberIds.join(", ")}. Leader
                        and workers must reference different agents.
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
                <div className="team-create-panel rounded-xl border border-slate-200 bg-slate-50/70 p-4">
                  <h4 className="text-base font-semibold text-slate-900">Launch Team</h4>
                  <p className="muted mt-2 text-sm text-slate-600">
                    Final review before deployment.
                  </p>
                  <div className="mono mt-3 grid min-w-0 gap-2 text-xs text-slate-700 sm:grid-cols-3">
                    <span className="rounded-lg border border-slate-200 bg-white px-3 py-2">
                      team={newTeamName.trim() || "-"}
                    </span>
                    <span className="rounded-lg border border-slate-200 bg-white px-3 py-2">
                      leader={leaderMemberId.trim() || "-"}
                    </span>
                    <span className="rounded-lg border border-slate-200 bg-white px-3 py-2">
                      workers={configuredWorkerCount}
                    </span>
                  </div>
                  {useSpecOverride ? (
                    <p className="muted mt-3 text-sm text-slate-600">
                      Manual Spec mode: edit full team spec JSON directly.
                    </p>
                  ) : (
                    <p className="muted mt-3 text-sm text-slate-600">
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

            <div className="modal-actions team-create-actions mt-4 flex flex-wrap items-center justify-end gap-2 border-t border-slate-200 pt-3">
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
