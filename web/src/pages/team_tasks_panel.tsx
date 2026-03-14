import React from "react";
import { SegmentedControl, TextInput } from "@mantine/core";
import type {
  TeamTaskRecord,
  TeamTaskRunCompilePreviewRecord,
  TeamTaskStatus,
} from "../api";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import {
  TEAM_LIST_ITEM_ACTIVE_CLASS,
  TEAM_LIST_ITEM_IDLE_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type TaskStatusFilter = "all" | TeamTaskStatus;

type TeamTasksPanelProps = {
  developerMode: boolean;
  tasks: TeamTaskRecord[];
  tasksLoading: boolean;
  selectedTaskId: string;
  onSelectedTaskIdChange: (taskId: string) => void;
  onRefreshTasks: () => Promise<void> | void;
  newTaskTitle: string;
  onNewTaskTitleChange: (value: string) => void;
  onCreateTask: () => Promise<void> | void;
  busy: string | null;
  compilePreviewContextId: string;
  onCompilePreviewContextIdChange: (value: string) => void;
  onCompileTaskRunPreview: () => Promise<void> | void;
  canCompileTask: boolean;
  compiledRunPreview: TeamTaskRunCompilePreviewRecord | null;
  onUseCompiledRunPayload: () => void;
  onCreateRunFromCompiledPreview: () => Promise<void> | void;
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
};

const TASKS_FILTER_BAR_CLASS =
  "rounded-lg border border-ui-border bg-ui-surface-soft/80 p-1";
const TASKS_WORKSPACE_GRID_CLASS =
  "mt-4 grid gap-4 xl:grid-cols-[minmax(260px,320px)_minmax(0,1fr)]";
const TASKS_LIST_PANEL_CLASS =
  "rounded-xl border border-ui-border bg-ui-surface-soft/70 p-3";
const TASKS_LIST_STACK_CLASS = "mt-3 flex max-h-[420px] flex-col gap-2 overflow-y-auto pr-1";
const TASKS_LIST_EMPTY_CLASS =
  "rounded-lg border border-dashed border-ui-border-strong bg-ui-surface px-3 py-3 text-sm text-ui-text-muted";
const TASKS_DETAIL_PANEL_CLASS =
  "rounded-xl border border-ui-border bg-ui-surface-soft/70 p-4";
const TASKS_DETAIL_META_CLASS =
  "mt-3 grid gap-2 text-sm text-ui-text-secondary sm:grid-cols-2 xl:grid-cols-3";
const TASKS_DETAIL_META_ITEM_CLASS =
  "rounded-lg border border-ui-border bg-ui-surface px-3 py-2";

const SEGMENTED_CONTROL_CLASSNAMES = {
  root: "rounded-lg border border-ui-border bg-ui-surface-soft/80 p-1",
  control: "flex-1",
  label:
    "rounded-md px-2.5 py-1.5 text-xs font-medium text-ui-text-muted transition data-[active]:bg-ui-surface data-[active]:text-ui-text-primary data-[active]:shadow-sm hover:text-ui-text-primary",
  indicator: "hidden",
  innerLabel: "truncate",
} as const;

const TASK_STATUS_FILTERS: ReadonlyArray<{ value: TaskStatusFilter; label: string }> = [
  { value: "all", label: "All" },
  { value: "open", label: "Open" },
  { value: "in_progress", label: "In progress" },
  { value: "completed", label: "Completed" },
  { value: "canceled", label: "Canceled" },
];

function resolveTaskStatusTone(status: TeamTaskStatus): StatusTone {
  switch (status) {
    case "in_progress":
      return "active";
    case "completed":
      return "active";
    case "canceled":
      return "danger";
    case "open":
    default:
      return "inactive";
  }
}

export function TeamTasksPanel(props: TeamTasksPanelProps) {
  const {
    developerMode,
    tasks,
    tasksLoading,
    selectedTaskId,
    onSelectedTaskIdChange,
    onRefreshTasks,
    newTaskTitle,
    onNewTaskTitleChange,
    onCreateTask,
    busy,
    compilePreviewContextId,
    onCompilePreviewContextIdChange,
    onCompileTaskRunPreview,
    canCompileTask,
    compiledRunPreview,
    onUseCompiledRunPayload,
    onCreateRunFromCompiledPreview,
    formatTs,
    toPrettyJson,
  } = props;
  const [statusFilter, setStatusFilter] = React.useState<TaskStatusFilter>("all");

  const visibleTasks = React.useMemo(() => {
    if (statusFilter === "all") {
      return tasks;
    }
    return tasks.filter((task) => task.status === statusFilter);
  }, [statusFilter, tasks]);

  const selectedTask = React.useMemo(
    () =>
      visibleTasks.find((task) => task.id === selectedTaskId) ??
      visibleTasks[0] ??
      null,
    [selectedTaskId, visibleTasks]
  );

  const canCreateTask = newTaskTitle.trim().length > 0 && busy !== "create-task";

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Tasks</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            type="button"
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            onClick={() => {
              void onRefreshTasks();
            }}
            disabled={tasksLoading}
            title="Refresh tasks"
            aria-label="Refresh tasks"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </button>
        </div>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <TextInput
          className="min-w-[220px] flex-1"
          placeholder="New task title"
          aria-label="New task title"
          value={newTaskTitle}
          onChange={(event) => onNewTaskTitleChange(event.currentTarget.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && canCreateTask) {
              event.preventDefault();
              void onCreateTask();
            }
          }}
          size="sm"
          radius="md"
        />
        <button
          type="button"
          className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
          onClick={() => {
            void onCreateTask();
          }}
          disabled={!canCreateTask}
        >
          New Task
        </button>
      </div>

      <div className={TASKS_FILTER_BAR_CLASS}>
        <SegmentedControl
          fullWidth
          size="xs"
          radius="md"
          value={statusFilter}
          onChange={(value) => setStatusFilter(value as TaskStatusFilter)}
          data={TASK_STATUS_FILTERS}
          aria-label="Task status filter"
          classNames={SEGMENTED_CONTROL_CLASSNAMES}
        />
      </div>

      <div className={TASKS_WORKSPACE_GRID_CLASS}>
        <div className={TASKS_LIST_PANEL_CLASS}>
          <p className="text-sm font-semibold text-ui-text-primary">Task list</p>
          <div className={TASKS_LIST_STACK_CLASS}>
            {tasksLoading && <div className={TASKS_LIST_EMPTY_CLASS}>Loading tasks...</div>}
            {!tasksLoading && visibleTasks.length === 0 && tasks.length === 0 && (
              <div className={TASKS_LIST_EMPTY_CLASS}>No tasks yet. Create one to plan work.</div>
            )}
            {!tasksLoading && visibleTasks.length === 0 && tasks.length > 0 && (
              <div className={TASKS_LIST_EMPTY_CLASS}>No tasks match the selected filter.</div>
            )}
            {visibleTasks.map((task) => (
              <button
                key={task.id}
                type="button"
                className={
                  task.id === selectedTask?.id
                    ? TEAM_LIST_ITEM_ACTIVE_CLASS
                    : TEAM_LIST_ITEM_IDLE_CLASS
                }
                onClick={() => onSelectedTaskIdChange(task.id)}
              >
                <span className="flex w-full items-start justify-between gap-2">
                  <span className={TEAM_LIST_ITEM_TITLE_CLASS}>{task.title}</span>
                  <StatusBadge
                    label={task.status}
                    tone={resolveTaskStatusTone(task.status)}
                    className="team-status"
                    title={`task status: ${task.status}`}
                  />
                </span>
                <span className={TEAM_LIST_ITEM_META_CLASS}>
                  {`updated=${formatTs(task.updated_at)} created=${formatTs(task.created_at)}`}
                </span>
              </button>
            ))}
          </div>
        </div>

        <div className={TASKS_DETAIL_PANEL_CLASS}>
          {!selectedTask && (
            <p className={TEAM_MUTED_TEXT_CLASS}>
              Select a task to inspect compile preview and run payload details.
            </p>
          )}

          {selectedTask && (
            <>
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <h4 className="text-base font-semibold text-ui-text-primary">{selectedTask.title}</h4>
                  <p className="mt-1 text-sm text-ui-text-muted">
                    Compile this task into a deterministic run payload preview, then reuse the
                    payload when creating a new run.
                  </p>
                </div>
                <StatusBadge
                  label={selectedTask.status}
                  tone={resolveTaskStatusTone(selectedTask.status)}
                  className="team-status"
                  title={`task status: ${selectedTask.status}`}
                />
              </div>

              <div className={TASKS_DETAIL_META_CLASS}>
                <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                  <strong>Task</strong>
                  <div className="mono mt-1 text-xs text-ui-text-muted">{selectedTask.id}</div>
                </div>
                <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                  <strong>Created</strong>
                  <div className="mt-1 text-xs text-ui-text-muted">{formatTs(selectedTask.created_at)}</div>
                </div>
                <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                  <strong>Updated</strong>
                  <div className="mt-1 text-xs text-ui-text-muted">{formatTs(selectedTask.updated_at)}</div>
                </div>
              </div>

              <div className="mt-4 flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
                  onClick={() => {
                    void onCompileTaskRunPreview();
                  }}
                  disabled={!canCompileTask}
                >
                  Compile Preview
                </button>
                <TextInput
                  className="min-w-[220px] flex-1"
                  placeholder="context_id override (optional)"
                  aria-label="context_id override"
                  value={compilePreviewContextId}
                  onChange={(event) => onCompilePreviewContextIdChange(event.currentTarget.value)}
                  size="sm"
                  radius="md"
                />
              </div>

              {compiledRunPreview ? (
                <div className="mt-4 space-y-3 rounded-xl border border-ui-border bg-ui-surface p-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <button
                      type="button"
                      className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
                      onClick={onUseCompiledRunPayload}
                    >
                      Use Payload in Create Run
                    </button>
                    <button
                      type="button"
                      className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
                      onClick={() => {
                        void onCreateRunFromCompiledPreview();
                      }}
                      disabled={busy === "create-run"}
                    >
                      Create Run from Preview
                    </button>
                  </div>
                  <div className="grid gap-2 text-sm text-ui-text-secondary sm:grid-cols-2">
                    <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                      <strong>Conversation</strong>
                      <div className="mono mt-1 text-xs text-ui-text-muted">
                        {compiledRunPreview.conversation_id}
                      </div>
                    </div>
                    <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                      <strong>Context</strong>
                      <div className="mono mt-1 text-xs text-ui-text-muted">
                        {compiledRunPreview.run_payload.context_id}
                      </div>
                    </div>
                  </div>
                  <pre className={TEAM_PANEL_PRE_CLASS}>
                    {toPrettyJson({
                      conversation_id: compiledRunPreview.conversation_id,
                      run_payload: compiledRunPreview.run_payload,
                      plan: compiledRunPreview.plan,
                    })}
                  </pre>
                </div>
              ) : (
                <p className={`${TEAM_MUTED_TEXT_CLASS} mt-4`}>
                  Compile preview not generated yet.
                </p>
              )}

              {developerMode &&
                selectedTask.context &&
                typeof selectedTask.context === "object" &&
                !Array.isArray(selectedTask.context) && (
                  <div className="mt-4">
                    <p className="text-xs font-semibold uppercase tracking-[0.14em] text-ui-text-muted">
                      Task context
                    </p>
                    <pre className={`${TEAM_PANEL_PRE_CLASS} mt-2`}>
                      {toPrettyJson(selectedTask.context)}
                    </pre>
                  </div>
                )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
