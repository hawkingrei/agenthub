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

export type TeamWorkspaceContextValue = {
  workbench?: TeamWorkbenchRuntimeContext;
  selectedConversation: TeamTaskRecord | null;
  developerMode: boolean;
  token: string;
  tasksLoading: boolean;
  onRefreshTasks: () => void;
  taskMessageDraft: string;
  setTaskMessageDraft: (val: string) => void;
  onSendTaskMessage: (payload: { text: string; mentionActorIds: string[] }) => void | Promise<void>;
  taskMessages: TeamConversationMessageRecord[];
  conversationMailboxMessages: TeamActorMessageRecord[];
  snapshot: TeamRunSnapshotRecord | null;
  mailboxDisplayNameByActorId: Record<string, string>;
  selectedTeamMemberLiveStates: TeamMemberLiveState[];
  taskConversationMemberIds: string[];
  activeConversationTitle: string;
  selectedConversationMatchesChannelLane: boolean;
  taskMessagesLoading: boolean;
  busy: string | null;
  routeThreadRootMessageId: number | null;
  channelFocusMessageId: number | null;
  setChannelFocusMessageId: (id: number | null) => void;
  effectiveSelectedTeamId: string | null;
  routeWorkspaceLens: WorkspaceLens | null;
  routeChannelId: string;
  activeChannelConversationTaskId: string | null;
  navigateTeamRoute: (path: string) => void;
  isCompactWorkbench: boolean;
  selectedChannelItem: TeamChannelItem | null | undefined;
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
  onSendThreadReply: (payload: { text: string; mentionActorIds: string[] }) => void;
  threadReplyDraft: string;
  setThreadReplyDraft: (val: string) => void;
};

const TeamWorkspaceContext = React.createContext<TeamWorkspaceContextValue | null>(null);

export function TeamWorkspaceProvider({
  value,
  children,
}: {
  value: TeamWorkspaceContextValue;
  children: React.ReactNode;
}) {
  return (
    <TeamWorkspaceContext.Provider value={value}>
      {children}
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
