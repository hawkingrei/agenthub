import React from "react";
import { TeamTasksPanel } from "../team_tasks_panel";
import { formatTs, toPrettyJson } from "./page_helpers";
import type { TeamChannelItem } from "./channel_metadata";

type TeamTasksPanelProps = React.ComponentProps<typeof TeamTasksPanel>;

type TeamTasksContainerProps = {
  isCompactWorkbench: boolean;
  selectedChannelItem: Pick<TeamChannelItem, "label">;
  developerMode: boolean;
  workspaceTasks: TeamTasksPanelProps["tasks"];
  tasksLoading: boolean;
  selectedTaskId: string;
  setSelectedTaskId: (id: string) => void;
  onRefreshTasks: () => void;
  onSelectConversationSubject: (taskId?: string | null, taskChannelId?: string | null) => void;
  busy: string | null;
  runs: TeamTasksPanelProps["runs"];
  onOpenTaskRun: (runId: string) => void;
  compilePreviewContextId: string;
  setCompilePreviewContextId: (id: string) => void;
  onCompileTaskRunPreview: () => void;
  canCompileTask: boolean;
  compiledRunPreview: TeamTasksPanelProps["compiledRunPreview"];
  onUseCompiledRunPayload: () => void;
  onCreateRunFromCompiledPreview: () => void;
  selectedTeamMemberLiveStates: TeamTasksPanelProps["memberLiveStates"];
};

export const TeamTasksContainer = React.memo(function TeamTasksContainer(
  props: TeamTasksContainerProps
) {
  const {
    isCompactWorkbench,
    selectedChannelItem,
    developerMode,
    workspaceTasks,
    tasksLoading,
    selectedTaskId,
    setSelectedTaskId,
    onRefreshTasks,
    onSelectConversationSubject,
    busy,
    runs,
    onOpenTaskRun,
    compilePreviewContextId,
    setCompilePreviewContextId,
    onCompileTaskRunPreview,
    canCompileTask,
    compiledRunPreview,
    onUseCompiledRunPayload,
    onCreateRunFromCompiledPreview,
    selectedTeamMemberLiveStates,
  } = props;

  return (
    <TeamTasksPanel
      compactMode={isCompactWorkbench}
      channelLabel={selectedChannelItem.label}
      developerMode={developerMode}
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
});
