import React from "react";
import { TeamTasksPanel } from "../team_tasks_panel";
import { formatTs, toPrettyJson } from "./page_helpers";
import { useTeamWorkspace } from "./team_workspace_context";

type TeamTasksPanelProps = React.ComponentProps<typeof TeamTasksPanel>;

export const TeamTasksContainer = React.memo(function TeamTasksContainer() {
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
  } = useTeamWorkspace();

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
      compiledRunPreview={compiledRunPreview as TeamTasksPanelProps["compiledRunPreview"]}
      onUseCompiledRunPayload={onUseCompiledRunPayload}
      onCreateRunFromCompiledPreview={onCreateRunFromCompiledPreview}
      formatTs={formatTs}
      toPrettyJson={toPrettyJson}
      memberLiveStates={selectedTeamMemberLiveStates}
    />
  );
});
