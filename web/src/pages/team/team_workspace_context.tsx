import React from "react";
import type { ComponentProps } from "react";
import type {
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
  TeamRunRecord,
  TeamRunSnapshotRecord,
  TeamTaskRecord,
} from "../../api";
import type { WorkspaceLens } from "../../app_route_selection";
import { TeamTasksPanel } from "../team_tasks_panel";
import type { TeamChannelItem } from "./channel_metadata";
import type { TeamMemberLiveState } from "./member_helpers";
import type { TeamWorkbenchRuntimeContext } from "./TeamWorkbenchContainer";

type TeamTasksPanelProps = ComponentProps<typeof TeamTasksPanel>;

export type TeamWorkspaceShellContextValue = {
  workbench?: TeamWorkbenchRuntimeContext;
  developerMode: boolean;
  busy: string | null;
  snapshot: TeamRunSnapshotRecord | null;
  mailboxDisplayNameByActorId: Record<string, string>;
  selectedTeamMemberLiveStates: TeamMemberLiveState[];
  selectedConversationMatchesChannelLane: boolean;
  routeThreadRootMessageId: number | null;
  effectiveSelectedTeamId: string | null;
  routeWorkspaceLens: WorkspaceLens | null;
  routeChannelId: string;
  activeChannelConversationTaskId: string | null;
  navigateTeamRoute: (path: string) => void;
  isCompactWorkbench: boolean;
  selectedChannelItem: TeamChannelItem | null | undefined;
};

export type TeamConversationContextValue = {
  selectedConversation: TeamTaskRecord | null;
  token: string;
  tasksLoading: boolean;
  onRefreshTasks: () => void;
  taskMessageDraft: string;
  setTaskMessageDraft: (val: string) => void;
  onSendTaskMessage: (payload: { text: string; mentionActorIds: string[] }) => void | Promise<void>;
  taskMessages: TeamConversationMessageRecord[];
  conversationMailboxMessages: TeamActorMessageRecord[];
  taskConversationMemberIds: string[];
  activeConversationTitle: string;
  taskMessagesLoading: boolean;
  channelFocusMessageId: number | null;
  setChannelFocusMessageId: (id: number | null) => void;
  onSendThreadReply: (payload: { text: string; mentionActorIds: string[] }) => void;
  threadReplyDraft: string;
  setThreadReplyDraft: (val: string) => void;
};

export type TeamTasksContextValue = {
  tasksLoading: boolean;
  onRefreshTasks: () => void;
  selectedTeamMemberLiveStates: TeamMemberLiveState[];
  isCompactWorkbench: boolean;
  selectedChannelItem: TeamChannelItem | null | undefined;
  developerMode: boolean;
  busy: string | null;
  workspaceTasks: TeamTaskRecord[];
  selectedTaskId: string;
  setSelectedTaskId: (id: string) => void;
  onSelectConversationSubject: (taskId?: string | null, taskChannelId?: string | null) => void;
  runs: TeamRunRecord[];
  onOpenTaskRun: (runId: string) => void;
  compilePreviewContextId: string;
  setCompilePreviewContextId: (id: string) => void;
  onCompileTaskRunPreview: () => void;
  canCompileTask: boolean;
  compiledRunPreview: TeamTasksPanelProps["compiledRunPreview"];
  onUseCompiledRunPayload: () => void;
  onCreateRunFromCompiledPreview: () => void;
};

export type TeamWorkspaceContextValue =
  TeamWorkspaceShellContextValue &
  TeamConversationContextValue &
  TeamTasksContextValue;

const TeamWorkspaceContext = React.createContext<TeamWorkspaceContextValue | null>(null);
const TeamWorkspaceShellContext = React.createContext<TeamWorkspaceShellContextValue | null>(null);
const TeamConversationContext = React.createContext<TeamConversationContextValue | null>(null);
const TeamTasksContext = React.createContext<TeamTasksContextValue | null>(null);

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
  const stableRef = React.useRef(value);
  if (!shallowEqualObject(stableRef.current, value)) {
    stableRef.current = value;
  }
  return stableRef.current;
}

export function TeamWorkspaceProvider({
  value,
  children,
}: {
  value: TeamWorkspaceContextValue;
  children: React.ReactNode;
}) {
  const shellValue = useShallowStableObject<TeamWorkspaceShellContextValue>({
    workbench: value.workbench,
    developerMode: value.developerMode,
    busy: value.busy,
    snapshot: value.snapshot,
    mailboxDisplayNameByActorId: value.mailboxDisplayNameByActorId,
    selectedTeamMemberLiveStates: value.selectedTeamMemberLiveStates,
    selectedConversationMatchesChannelLane: value.selectedConversationMatchesChannelLane,
    routeThreadRootMessageId: value.routeThreadRootMessageId,
    effectiveSelectedTeamId: value.effectiveSelectedTeamId,
    routeWorkspaceLens: value.routeWorkspaceLens,
    routeChannelId: value.routeChannelId,
    activeChannelConversationTaskId: value.activeChannelConversationTaskId,
    navigateTeamRoute: value.navigateTeamRoute,
    isCompactWorkbench: value.isCompactWorkbench,
    selectedChannelItem: value.selectedChannelItem,
  });
  const conversationValue = useShallowStableObject<TeamConversationContextValue>({
    selectedConversation: value.selectedConversation,
    token: value.token,
    tasksLoading: value.tasksLoading,
    onRefreshTasks: value.onRefreshTasks,
    taskMessageDraft: value.taskMessageDraft,
    setTaskMessageDraft: value.setTaskMessageDraft,
    onSendTaskMessage: value.onSendTaskMessage,
    taskMessages: value.taskMessages,
    conversationMailboxMessages: value.conversationMailboxMessages,
    taskConversationMemberIds: value.taskConversationMemberIds,
    activeConversationTitle: value.activeConversationTitle,
    taskMessagesLoading: value.taskMessagesLoading,
    channelFocusMessageId: value.channelFocusMessageId,
    setChannelFocusMessageId: value.setChannelFocusMessageId,
    onSendThreadReply: value.onSendThreadReply,
    threadReplyDraft: value.threadReplyDraft,
    setThreadReplyDraft: value.setThreadReplyDraft,
  });
  const tasksValue = useShallowStableObject<TeamTasksContextValue>({
    tasksLoading: value.tasksLoading,
    onRefreshTasks: value.onRefreshTasks,
    selectedTeamMemberLiveStates: value.selectedTeamMemberLiveStates,
    isCompactWorkbench: value.isCompactWorkbench,
    selectedChannelItem: value.selectedChannelItem,
    developerMode: value.developerMode,
    busy: value.busy,
    workspaceTasks: value.workspaceTasks,
    selectedTaskId: value.selectedTaskId,
    setSelectedTaskId: value.setSelectedTaskId,
    onSelectConversationSubject: value.onSelectConversationSubject,
    runs: value.runs,
    onOpenTaskRun: value.onOpenTaskRun,
    compilePreviewContextId: value.compilePreviewContextId,
    setCompilePreviewContextId: value.setCompilePreviewContextId,
    onCompileTaskRunPreview: value.onCompileTaskRunPreview,
    canCompileTask: value.canCompileTask,
    compiledRunPreview: value.compiledRunPreview,
    onUseCompiledRunPayload: value.onUseCompiledRunPayload,
    onCreateRunFromCompiledPreview: value.onCreateRunFromCompiledPreview,
  });

  return (
    <TeamWorkspaceContext.Provider value={value}>
      <TeamWorkspaceShellContext.Provider value={shellValue}>
        <TeamConversationContext.Provider value={conversationValue}>
          <TeamTasksContext.Provider value={tasksValue}>{children}</TeamTasksContext.Provider>
        </TeamConversationContext.Provider>
      </TeamWorkspaceShellContext.Provider>
    </TeamWorkspaceContext.Provider>
  );
}

export function useTeamWorkspace(): TeamWorkspaceContextValue {
  const value = React.useContext(TeamWorkspaceContext);
  if (!value) {
    throw new Error("useTeamWorkspace must be used within TeamWorkspaceProvider");
  }
  return value;
}

export function useTeamWorkspaceShell(): TeamWorkspaceShellContextValue {
  const value = React.useContext(TeamWorkspaceShellContext);
  if (!value) {
    throw new Error("useTeamWorkspaceShell must be used within TeamWorkspaceProvider");
  }
  return value;
}

export function useTeamConversationContext(): TeamConversationContextValue {
  const value = React.useContext(TeamConversationContext);
  if (!value) {
    throw new Error("useTeamConversationContext must be used within TeamWorkspaceProvider");
  }
  return value;
}

export function useTeamTasksContext(): TeamTasksContextValue {
  const value = React.useContext(TeamTasksContext);
  if (!value) {
    throw new Error("useTeamTasksContext must be used within TeamWorkspaceProvider");
  }
  return value;
}
