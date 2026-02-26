import { DEFAULT_AGENT_PRESET_ID, type AgentPresetId } from "../../agent_presets";
import type { TeamActorMessageRecord } from "../../api";
import type { MailboxTemplateKey } from "./mailbox_helpers";
import { createInitialTeamDraftState, type TeamCreateDraftState } from "./member_helpers";
import type { TeamRunStatusFilter } from "./run_helpers";

export type TeamTab =
  | "runs"
  | "conversation"
  | "agent_acp"
  | "overview"
  | "events"
  | "steps"
  | "mailbox"
  | "member_console"
  | "debug";

export const TEAM_TAB_ITEMS: ReadonlyArray<{ value: TeamTab; label: string }> = [
  { value: "runs", label: "Runs" },
  { value: "conversation", label: "Conversation" },
  { value: "agent_acp", label: "Agent ACP" },
  { value: "overview", label: "Overview" },
  { value: "events", label: "Events" },
  { value: "steps", label: "Steps" },
  { value: "mailbox", label: "Mailbox" },
  { value: "member_console", label: "Member Console" },
  { value: "debug", label: "Debug" },
];

const TEAM_TABS_WITHOUT_ACTIVE_RUN = new Set<TeamTab>(["runs", "conversation", "debug"]);

export function tabRequiresActiveRun(tab: TeamTab): boolean {
  return !TEAM_TABS_WITHOUT_ACTIVE_RUN.has(tab);
}
export type CreateTeamStage = 0 | 1 | 2 | 3;
export type TeamForgeRoleTag = "leader" | "worker";

export type TeamRunBrowserState = {
  statusFilter: TeamRunStatusFilter;
  beforeCreatedAt?: number;
  hasMore: boolean;
};

export type StepAction = "start" | "complete" | "fail" | "input_required" | "resume";

export type TeamUiState = {
  tab: TeamTab;
  runLookupId: string;
  eventsAutoRefresh: boolean;
};

export type TeamUiAction =
  | { type: "set_tab"; tab: TeamTab }
  | { type: "set_run_lookup_id"; runLookupId: string }
  | { type: "set_events_auto_refresh"; eventsAutoRefresh: boolean };

export type TeamControlState = {
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

export type TeamControlAction = { type: "patch"; patch: Partial<TeamControlState> };

export type TeamMailboxState = {
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

export type TeamMailboxAction =
  | { type: "patch"; patch: Partial<TeamMailboxState> }
  | { type: "mark_conversation_seen"; key: string; messageId: number }
  | { type: "reset_chat_seen" };

export type TeamCreateState = TeamCreateDraftState & {
  newTeamName: string;
  newTeamDescription: string;
  showCreateTeamModal: boolean;
  createTeamStage: CreateTeamStage;
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

export type TeamCreateAction = { type: "patch"; patch: Partial<TeamCreateState> };

export const EVENT_PAGE_LIMIT = 100;
export const MEMBER_EVENT_PAGE_LIMIT = 300;
export const TEAM_RUN_PAGE_LIMIT = 50;
export const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";

export const DEFAULT_TEAM_RUN_BROWSER_STATE: TeamRunBrowserState = {
  statusFilter: "all",
  hasMore: false,
};

export const DEFAULT_TEAM_UI_STATE: TeamUiState = {
  tab: "conversation",
  runLookupId: "",
  eventsAutoRefresh: true,
};

export const DEFAULT_TEAM_CONTROL_STATE: TeamControlState = {
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

export const DEFAULT_TEAM_MAILBOX_STATE: TeamMailboxState = {
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

export const MAILBOX_TEMPLATE_OPTIONS: Array<{
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

export const TEAM_RUN_STATUS_FILTER_OPTIONS: Array<{
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

export const CREATE_TEAM_STAGE_TITLES = [
  "Mission Brief",
  "Leader Forge",
  "Recruit Workers",
  "Launch Team",
] as const;

export function reduceTeamUiState(state: TeamUiState, action: TeamUiAction): TeamUiState {
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

export function reduceTeamControlState(
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

export function reduceTeamMailboxState(
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

export function reduceTeamCreateState(
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

export function resolveUpdater<T>(current: T, next: T | ((prev: T) => T)): T {
  if (typeof next === "function") {
    return (next as (prev: T) => T)(current);
  }
  return next;
}

export function createInitialTeamCreateState(): TeamCreateState {
  const draft = createInitialTeamDraftState();
  return {
    ...draft,
    newTeamName: "",
    newTeamDescription: "",
    showCreateTeamModal: false,
    createTeamStage: 0,
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
