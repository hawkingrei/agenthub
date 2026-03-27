import React from "react";
import { SegmentedControl, TextInput } from "@mantine/core";
import type {
  TeamTaskRecord,
  TeamTaskRunCompilePreviewRecord,
  TeamRunRecord,
  TeamTaskStatus,
} from "../api";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import { selectRunsForTask } from "./team/run_helpers";
import type { TeamMemberLiveState } from "./team/member_helpers";
import {
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
  runs: TeamRunRecord[];
  onOpenRun: (runId: string) => void;
  compilePreviewContextId: string;
  onCompilePreviewContextIdChange: (value: string) => void;
  onCompileTaskRunPreview: () => Promise<void> | void;
  canCompileTask: boolean;
  compiledRunPreview: TeamTaskRunCompilePreviewRecord | null;
  onUseCompiledRunPayload: () => void;
  onCreateRunFromCompiledPreview: () => Promise<void> | void;
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
  memberLiveStates: TeamMemberLiveState[];
};

const TASKS_FILTER_BAR_CLASS =
  "rounded-[14px] border border-ui-border bg-ui-surface-soft/70 p-0.5";
const TASKS_WORKSPACE_STACK_CLASS = "mt-3.5 flex flex-col gap-3.5";
const TASKS_BOARD_SCROLL_CLASS = "-mx-1 overflow-x-auto px-1 pb-1";
const TASKS_BOARD_LANES_CLASS =
  "grid min-w-full auto-cols-[minmax(220px,1fr)] grid-flow-col gap-3";
const TASKS_BOARD_COLUMN_CLASS =
  "flex min-h-[300px] flex-col rounded-[16px] border border-ui-border bg-ui-surface-soft/55 p-2.5";
const TASKS_BOARD_COLUMN_HEADER_CLASS =
  "flex items-start justify-between gap-2.5 border-b border-ui-border/80 pb-2";
const TASKS_BOARD_COLUMN_META_CLASS =
  "text-[10px] font-medium uppercase tracking-[0.12em] text-ui-text-muted";
const TASKS_BOARD_STACK_CLASS = "mt-2.5 flex min-h-0 flex-1 flex-col gap-2";
const TASKS_BOARD_EMPTY_CLASS =
  "rounded-[14px] border border-dashed border-ui-border-strong bg-ui-surface px-2.5 py-2.5 text-sm text-ui-text-muted";
const TASKS_BOARD_CARD_ACTIVE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-1.5 rounded-[14px] border border-ui-border-strong bg-ui-surface px-2.5 py-2.5 text-left text-ui-text-primary shadow-sm transition";
const TASKS_BOARD_CARD_IDLE_CLASS =
  "team-item flex w-full min-w-0 flex-col items-start gap-1.5 rounded-[14px] border border-ui-border bg-ui-surface px-2.5 py-2.5 text-left text-ui-text-primary shadow-sm transition hover:border-ui-border-strong hover:bg-ui-surface-soft";
const TASKS_BOARD_CARD_META_ROW_CLASS =
  "flex w-full items-center justify-between gap-2 text-[11px] text-ui-text-muted";
const TASKS_BOARD_CARD_SELECT_BUTTON_CLASS =
  "flex w-full min-w-0 flex-col items-start gap-1.5 text-left";
const TASKS_DETAIL_PANEL_CLASS =
  "rounded-[18px] border border-ui-border bg-ui-surface p-3.5 shadow-sm";
const TASKS_DETAIL_META_CLASS =
  "mt-3 grid gap-2 text-sm text-ui-text-secondary sm:grid-cols-2 xl:grid-cols-3";
const TASKS_DETAIL_META_ITEM_CLASS =
  "rounded-[14px] border border-ui-border bg-ui-surface-soft/65 px-2.5 py-2";
const TASKS_RUN_CARD_CLASS =
  "rounded-[16px] border border-ui-border bg-ui-surface-soft/60 px-2.5 py-2.5";
const TASKS_RUN_LIST_CLASS = "mt-3.5 space-y-2";
const TASKS_DEBUG_DISCLOSURE_CLASS =
  "mt-3.5 rounded-[16px] border border-dashed border-ui-border bg-ui-surface-soft/55 px-2.5 py-2.5";
const TASKS_DEBUG_SUMMARY_CLASS =
  "flex cursor-pointer list-none items-center justify-between gap-3 text-left";
const TASKS_DEBUG_SUMMARY_META_CLASS = "mt-1 text-sm text-ui-text-muted";
const TASKS_DEBUG_BODY_CLASS = "mt-3 space-y-3 border-t border-ui-border pt-3";

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
  { value: "in_review", label: "In review" },
  { value: "completed", label: "Completed" },
  { value: "canceled", label: "Canceled" },
];
const TASK_BOARD_COLUMNS: ReadonlyArray<{
  status: TeamTaskStatus;
  label: string;
  description: string;
}> = [
  { status: "open", label: "Open", description: "Queued and not started yet." },
  {
    status: "in_progress",
    label: "In progress",
    description: "Actively being worked on now.",
  },
  {
    status: "in_review",
    label: "In review",
    description: "Implementation finished and waiting for review.",
  },
  { status: "completed", label: "Completed", description: "Reviewed and accepted." },
  { status: "canceled", label: "Canceled", description: "Stopped or intentionally dropped." },
] as const;

function resolveTaskStatusTone(status: TeamTaskStatus): StatusTone {
  switch (status) {
    case "in_progress":
      return "active";
    case "in_review":
      return "warning";
    case "completed":
      return "active";
    case "canceled":
      return "danger";
    case "open":
    default:
      return "inactive";
  }
}

function formatTaskEmptyLabel(status: TeamTaskStatus): string {
  if (status === "in_progress") {
    return "No tasks in progress.";
  }
  if (status === "in_review") {
    return "No tasks waiting for review.";
  }
  if (status === "completed") {
    return "No completed tasks yet.";
  }
  if (status === "canceled") {
    return "No canceled tasks.";
  }
  return "No open tasks.";
}

function resolveRunStatusTone(status: TeamRunRecord["status"]): StatusTone {
  switch (status) {
    case "working":
    case "completed":
      return "active";
    case "canceled":
      return "inactive";
    case "failed":
      return "danger";
    case "input_required":
      return "warning";
    case "submitted":
    default:
      return "inactive";
  }
}

function resolveTaskAssigneeLabel(
  task: TeamTaskRecord,
  assigneeLabelById: Map<string, string>
): string {
  const assignedMemberId = task.assigned_member_id?.trim();
  if (!assignedMemberId) {
    return "Unassigned";
  }
  return assigneeLabelById.get(assignedMemberId) ?? assignedMemberId;
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
    runs,
    onOpenRun,
    compilePreviewContextId,
    onCompilePreviewContextIdChange,
    onCompileTaskRunPreview,
    canCompileTask,
    compiledRunPreview,
    onUseCompiledRunPayload,
    onCreateRunFromCompiledPreview,
    formatTs,
    toPrettyJson,
    memberLiveStates,
  } = props;
  const [statusFilter, setStatusFilter] = React.useState<TaskStatusFilter>("all");
  const [debugToolsOpen, setDebugToolsOpen] = React.useState(false);

  const visibleTasks = React.useMemo(() => {
    if (statusFilter === "all") {
      return tasks;
    }
    return tasks.filter((task) => task.status === statusFilter);
  }, [statusFilter, tasks]);
  const visibleColumns = React.useMemo(() => {
    if (statusFilter === "all") {
      return TASK_BOARD_COLUMNS;
    }
    return TASK_BOARD_COLUMNS.filter((column) => column.status === statusFilter);
  }, [statusFilter]);
  const visibleTasksByStatus = React.useMemo(() => {
    const grouped = new Map<TeamTaskStatus, TeamTaskRecord[]>(
      TASK_BOARD_COLUMNS.map((column) => [column.status, []])
    );
    for (const task of visibleTasks) {
      grouped.get(task.status)?.push(task);
    }
    return grouped;
  }, [visibleTasks]);

  const selectedTask = React.useMemo(
    () =>
      visibleTasks.find((task) => task.id === selectedTaskId) ??
      visibleTasks[0] ??
      null,
    [selectedTaskId, visibleTasks]
  );

  const canCreateTask = newTaskTitle.trim().length > 0 && busy !== "create-task";
  const assigneeLabelById = React.useMemo(
    () =>
      new Map(
        memberLiveStates.map((member) => [
          member.member_id,
          member.agent_name?.trim() || member.member_id,
        ])
      ),
    [memberLiveStates]
  );
  const relatedRuns = React.useMemo(
    () => (selectedTask ? selectRunsForTask(runs, selectedTask.id) : []),
    [runs, selectedTask]
  );
  const latestRun = relatedRuns[0] ?? null;

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Kanban</h3>
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

      <div className={TASKS_WORKSPACE_STACK_CLASS}>
        <div className="space-y-2.5">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <p className="text-sm font-semibold text-ui-text-primary">Board lanes</p>
              <p className="mt-0.5 text-xs text-ui-text-muted">
                Grouped by task state.
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2 text-xs text-ui-text-muted">
              <span>{`${visibleColumns.length} lanes`}</span>
              <span aria-hidden="true">·</span>
              <span>{`${visibleTasks.length} tasks`}</span>
            </div>
          </div>

          <div className={TASKS_BOARD_SCROLL_CLASS}>
            <div className={TASKS_BOARD_LANES_CLASS}>
              {visibleColumns.map((column) => {
                const laneTasks = visibleTasksByStatus.get(column.status) ?? [];
                return (
                  <section key={column.status} className={TASKS_BOARD_COLUMN_CLASS}>
                    <div className={TASKS_BOARD_COLUMN_HEADER_CLASS}>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <h4 className="text-sm font-semibold text-ui-text-primary">
                            {column.label}
                          </h4>
                          <span className="rounded-full border border-ui-border bg-ui-surface px-2 py-0.5 text-xs font-semibold text-ui-text-secondary">
                            {laneTasks.length}
                          </span>
                        </div>
                        <p className={`mt-1 ${TASKS_BOARD_COLUMN_META_CLASS}`}>
                          {column.description}
                        </p>
                      </div>
                      <StatusBadge
                        label={column.status}
                        tone={resolveTaskStatusTone(column.status)}
                        className="team-status"
                        title={`lane status: ${column.status}`}
                      />
                    </div>

                    <div className={TASKS_BOARD_STACK_CLASS}>
                      {tasksLoading && (
                        <div className={TASKS_BOARD_EMPTY_CLASS}>Loading tasks...</div>
                      )}
                      {!tasksLoading && tasks.length === 0 && (
                        <div className={TASKS_BOARD_EMPTY_CLASS}>
                          No tasks yet. Create one to plan work.
                        </div>
                      )}
                      {!tasksLoading && tasks.length > 0 && laneTasks.length === 0 && (
                        <div className={TASKS_BOARD_EMPTY_CLASS}>
                          {statusFilter === "all"
                            ? formatTaskEmptyLabel(column.status)
                            : "No tasks match the selected filter."}
                        </div>
                      )}
                      {!tasksLoading &&
                        laneTasks.map((task) => (
                          <div
                            key={task.id}
                            className={
                              task.id === selectedTask?.id
                                ? TASKS_BOARD_CARD_ACTIVE_CLASS
                                : TASKS_BOARD_CARD_IDLE_CLASS
                            }
                          >
                            <button
                              type="button"
                              className={TASKS_BOARD_CARD_SELECT_BUTTON_CLASS}
                              onClick={() => onSelectedTaskIdChange(task.id)}
                            >
                              <span className={TEAM_LIST_ITEM_TITLE_CLASS}>{task.title}</span>
                              <span className={TASKS_BOARD_CARD_META_ROW_CLASS}>
                                <span>{`updated ${formatTs(task.updated_at)}`}</span>
                                <span>{`created ${formatTs(task.created_at)}`}</span>
                              </span>
                              <span className={TEAM_LIST_ITEM_META_CLASS}>
                                {`owner ${resolveTaskAssigneeLabel(task, assigneeLabelById)}`}
                              </span>
                              {developerMode && (
                                <span className={TEAM_LIST_ITEM_META_CLASS}>{task.id}</span>
                              )}
                            </button>
                          </div>
                        ))}
                    </div>
                  </section>
                );
              })}
            </div>
          </div>
        </div>

        <div className={TASKS_DETAIL_PANEL_CLASS}>
          {!selectedTask && (
            <p className={TEAM_MUTED_TEXT_CLASS}>
              Select a task to inspect linked runs and the latest execution summary.
            </p>
          )}

          {selectedTask && (
            <>
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <h4 className="text-[15px] font-semibold text-ui-text-primary sm:text-base">
                    {selectedTask.title}
                  </h4>
                  <p className="mt-1 text-sm text-ui-text-muted">
                    Agents pick this task up automatically. Runs capture the execution timeline,
                    and successful attempts move the task into review before final completion.
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
                  <strong>Assignee</strong>
                  <div className="mt-1 text-xs text-ui-text-muted">
                    {resolveTaskAssigneeLabel(selectedTask, assigneeLabelById)}
                  </div>
                </div>
                <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                  <strong>Updated</strong>
                  <div className="mt-1 text-xs text-ui-text-muted">{formatTs(selectedTask.updated_at)}</div>
                </div>
              </div>

              <div className="mt-4 rounded-[14px] border border-ui-border bg-ui-surface-soft/65 px-2.5 py-2 text-sm text-ui-text-muted">
                Task status and ownership are agent-managed through Team runtime controls.
              </div>

              <div className={TASKS_RUN_LIST_CLASS}>
                <div className={TASKS_RUN_CARD_CLASS}>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <p className="text-sm font-semibold text-ui-text-primary">Latest run</p>
                      <p className="mt-1 text-sm text-ui-text-muted">
                        {latestRun
                          ? latestRun.summary?.trim() ||
                            (latestRun.status === "completed"
                              ? "Completed without a structured summary."
                              : "Execution summary will appear here when the run finishes.")
                          : "No run recorded yet. The task will execute automatically once the team is ready."}
                      </p>
                    </div>
                    {latestRun && (
                      <StatusBadge
                        label={latestRun.status}
                        tone={resolveRunStatusTone(latestRun.status)}
                        className="team-status"
                        title={`run status: ${latestRun.status}`}
                      />
                    )}
                  </div>
                  {latestRun && (
                    <>
                      <div className="mt-3 grid gap-2 text-sm text-ui-text-secondary sm:grid-cols-2 xl:grid-cols-4">
                        <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                          <strong>Run</strong>
                          <div className="mono mt-1 text-xs text-ui-text-muted">{latestRun.id}</div>
                        </div>
                        <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                          <strong>Context</strong>
                          <div className="mono mt-1 text-xs text-ui-text-muted">
                            {latestRun.context_id}
                          </div>
                        </div>
                        <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                          <strong>Started</strong>
                          <div className="mt-1 text-xs text-ui-text-muted">
                            {formatTs(latestRun.started_at ?? latestRun.created_at)}
                          </div>
                        </div>
                        <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                          <strong>Ended</strong>
                          <div className="mt-1 text-xs text-ui-text-muted">
                            {latestRun.ended_at ? formatTs(latestRun.ended_at) : "running"}
                          </div>
                        </div>
                      </div>
                      <div className="mt-3 flex flex-wrap items-center gap-2">
                        <button
                          type="button"
                          className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
                          onClick={() => onOpenRun(latestRun.id)}
                        >
                          Open Run
                        </button>
                      </div>
                    </>
                  )}
                </div>

                {relatedRuns.length > 1 && (
                  <div className={TASKS_RUN_CARD_CLASS}>
                    <div className="flex items-center justify-between gap-3">
                      <div>
                        <p className="text-sm font-semibold text-ui-text-primary">Previous runs</p>
                        <p className="mt-1 text-sm text-ui-text-muted">
                          Earlier execution attempts linked to this task.
                        </p>
                      </div>
                      <span className="rounded-full border border-ui-border bg-ui-surface-soft px-2 py-0.5 text-xs font-semibold text-ui-text-secondary">
                        {relatedRuns.length - 1}
                      </span>
                    </div>
                    <div className="mt-3 space-y-2">
                      {relatedRuns.slice(1, 4).map((run) => (
                        <button
                          key={run.id}
                          type="button"
                          className="flex w-full flex-col items-start gap-2 rounded-lg border border-ui-border bg-ui-surface px-3 py-2 text-left transition hover:border-ui-border-strong hover:bg-ui-surface-soft"
                          onClick={() => onOpenRun(run.id)}
                        >
                          <div className="flex w-full flex-wrap items-center justify-between gap-2">
                            <span className="mono text-xs text-ui-text-muted">{run.id}</span>
                            <StatusBadge
                              label={run.status}
                              tone={resolveRunStatusTone(run.status)}
                              className="team-status"
                              title={`run status: ${run.status}`}
                            />
                          </div>
                          <span className="text-sm text-ui-text-primary">
                            {run.summary?.trim() || "No structured summary recorded."}
                          </span>
                          <span className="text-xs text-ui-text-muted">
                            {`created ${formatTs(run.created_at)}`}
                          </span>
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>

              {developerMode && (
                <details
                  className={TASKS_DEBUG_DISCLOSURE_CLASS}
                  open={debugToolsOpen}
                  onToggle={(event) => {
                    setDebugToolsOpen((event.currentTarget as HTMLDetailsElement).open);
                  }}
                >
                  <summary className={TASKS_DEBUG_SUMMARY_CLASS}>
                    <div>
                      <p className="text-sm font-semibold text-ui-text-primary">Developer tools</p>
                      <p className={TASKS_DEBUG_SUMMARY_META_CLASS}>
                        Manual compile preview and raw task context stay available here without
                        taking over the task detail surface.
                      </p>
                    </div>
                    <span className="shrink-0 text-xs font-semibold uppercase tracking-[0.12em] text-ui-text-muted">
                      {debugToolsOpen ? "Hide" : "Show"}
                    </span>
                  </summary>
                  <div className={TASKS_DEBUG_BODY_CLASS}>
                    <div className="flex flex-wrap items-center gap-2">
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
                        onChange={(event) =>
                          onCompilePreviewContextIdChange(event.currentTarget.value)
                        }
                        size="sm"
                        radius="md"
                      />
                    </div>

                    {compiledRunPreview ? (
                      <div className="space-y-3 rounded-xl border border-ui-border bg-ui-surface-soft p-3">
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
                      <p className={TEAM_MUTED_TEXT_CLASS}>Compile preview not generated yet.</p>
                    )}

                    {selectedTask.context &&
                      typeof selectedTask.context === "object" &&
                      !Array.isArray(selectedTask.context) && (
                        <div>
                          <p className="text-xs font-semibold uppercase tracking-[0.14em] text-ui-text-muted">
                            Task context
                          </p>
                          <pre className={`${TEAM_PANEL_PRE_CLASS} mt-2`}>
                            {toPrettyJson(selectedTask.context)}
                          </pre>
                        </div>
                      )}
                  </div>
                </details>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
