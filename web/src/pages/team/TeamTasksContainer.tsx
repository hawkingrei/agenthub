import React from "react";
import { TeamTasksPanel } from "../team_tasks_panel";
import { formatTs, toPrettyJson } from "./page_helpers";
import { useTeamTasksContext, useTeamWorkspaceShell } from "./team_workspace_context";

export const TeamTasksContainer = React.memo(function TeamTasksContainer() {
  const {
    workspaceTasks,
    tasksLoading,
    selectedTaskId,
    selectedTaskDetail,
    setSelectedTaskId,
    onRefreshTasks,
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
  } = useTeamTasksContext();
  const {
    isCompactWorkbench,
    selectedChannelItem,
    developerMode,
    busy,
    selectedTeamMemberLiveStates,
  } = useTeamWorkspaceShell();

  return (
    <TeamTasksPanel
      compactMode={isCompactWorkbench}
      channelLabel={selectedChannelItem?.label ?? "# all"}
      developerMode={developerMode}
      tasks={workspaceTasks}
      tasksLoading={tasksLoading}
      selectedTaskId={selectedTaskId}
      selectedTaskDetail={selectedTaskDetail}
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
