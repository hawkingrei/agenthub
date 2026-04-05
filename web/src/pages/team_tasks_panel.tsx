import React from "react";
import { CloseButton, SegmentedControl, TextInput } from "@mantine/core";
import type {
  TeamTaskRecord,
  TeamTaskRunCompilePreviewRecord,
  TeamRunRecord,
  TeamTaskStatus,
} from "../api";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import {
  ActionButton,
  SelectableListItem,
  ToolbarRow,
} from "../ui/primitives";
import { selectRunsForTask } from "./team/run_helpers";
import type { TeamMemberLiveState } from "./team/member_helpers";
import {
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
  TASKS_BOARD_LANES_CLASS,
  TASKS_BOARD_COLUMN_CLASS,
  TASKS_BOARD_COLUMN_HEADER_CLASS,
  TASKS_BOARD_CARD_CLASS,
  TASKS_BOARD_CARD_ACTIVE_CLASS,
  TASKS_DETAIL_PANEL_CLASS,
  TASKS_DETAIL_META_ITEM_CLASS,
} from "../ui/tailwind_classes";

type TaskStatusFilter = "all" | TeamTaskStatus;

type TeamTasksPanelProps = {
  compactMode?: boolean;
  developerMode: boolean;
  tasks: TeamTaskRecord[];
  tasksLoading: boolean;
  selectedTaskId: string;
  onSelectedTaskIdChange: (taskId: string) => void;
  onRefreshTasks: () => Promise<void> | void;
  onOpenConversation: (taskId?: string | null) => void;
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
  "rounded-lg border border-notion-border bg-notion-sidebar/50 p-1 shadow-sm";
const TASKS_WORKSPACE_STACK_CLASS = "mt-6 flex flex-col gap-8";
const TASKS_BOARD_SCROLL_CLASS = "-mx-1 overflow-x-auto px-1 pb-4";
const TASKS_BOARD_COLUMN_META_CLASS =
  "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";
const TASKS_BOARD_STACK_CLASS = "mt-4 flex min-h-0 flex-1 flex-col gap-2";
const TASKS_BOARD_EMPTY_CLASS =
  "rounded-md border border-dashed border-notion-border bg-white/50 px-3 py-4 text-sm text-notion-text-muted italic text-center";
const TASKS_BOARD_CARD_IDLE_CLASS = TASKS_BOARD_CARD_CLASS;
const TASKS_BOARD_CARD_META_ROW_CLASS =
  "flex w-full items-center justify-between gap-2 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted opacity-70";
const TASKS_BOARD_CARD_SELECT_BUTTON_CLASS =
  "flex w-full min-w-0 flex-col items-start gap-1.5 text-left";
const TASKS_DETAIL_META_CLASS =
  "mt-4 grid gap-3 text-sm text-notion-text sm:grid-cols-2 xl:grid-cols-3";
const TASKS_RUN_CARD_CLASS =
  "rounded-lg border border-notion-border bg-notion-sidebar/20 p-4 transition-all";
const TASKS_RUN_LIST_CLASS = "mt-6 space-y-3";
const TASKS_DEBUG_DISCLOSURE_CLASS =
  "mt-6 rounded-lg border border-dashed border-notion-border bg-notion-sidebar/10 p-4 transition-all";
const TASKS_DEBUG_SUMMARY_CLASS =
  "flex cursor-pointer list-none items-center justify-between gap-3 text-left";
const TASKS_DEBUG_SUMMARY_META_CLASS = "mt-1 text-[13px] text-notion-text-muted";
const TASKS_DEBUG_BODY_CLASS = "mt-4 space-y-4 border-t border-notion-border/50 pt-4";

const SEGMENTED_CONTROL_CLASSNAMES = {
  root: "rounded-md border border-notion-border bg-white/50 p-0.5",
  control: "flex-1",
  label:
    "rounded-sm px-2 py-1 text-[11px] font-bold uppercase tracking-wider text-notion-text-muted transition data-[active]:bg-white data-[active]:text-notion-text data-[active]:shadow-sm hover:text-notion-text",
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

function TeamTasksPanelImpl(props: TeamTasksPanelProps) {
  const {
    compactMode = false,
    developerMode,
    tasks,
    tasksLoading,
    selectedTaskId,
    onSelectedTaskIdChange,
    onRefreshTasks,
    onOpenConversation,
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
  const [compactDetailOpen, setCompactDetailOpen] = React.useState(false);

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
  const showInitialLoadingState = tasksLoading && tasks.length === 0;

  React.useEffect(() => {
    if (!compactMode) {
      setCompactDetailOpen(false);
    }
  }, [compactMode]);

  React.useEffect(() => {
    if (!compactMode) {
      return;
    }
    if (!selectedTaskId.trim()) {
      setCompactDetailOpen(false);
      return;
    }
    if (!visibleTasks.some((task) => task.id === selectedTaskId)) {
      setCompactDetailOpen(false);
    }
  }, [compactMode, selectedTaskId, visibleTasks]);

  const onSelectTask = React.useCallback(
    (taskId: string) => {
      onSelectedTaskIdChange(taskId);
      if (compactMode) {
        setCompactDetailOpen(true);
      }
    },
    [compactMode, onSelectedTaskIdChange]
  );

  const boardPanel = (
    <div className="space-y-4">
      <ToolbarRow>
        <div>
          <p className="text-sm font-bold text-notion-text uppercase tracking-widest">Board lanes</p>
          <p className="mt-1 text-[12px] text-notion-text-muted">
            Grouped by task state.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-[11px] font-bold uppercase tracking-wider text-notion-text-muted">
          <span>{`${visibleColumns.length} lanes`}</span>
          <span aria-hidden="true">·</span>
          <span>{`${visibleTasks.length} tasks`}</span>
        </div>
      </ToolbarRow>

      <div className={TASKS_BOARD_SCROLL_CLASS}>
        <div className={TASKS_BOARD_LANES_CLASS}>
          {visibleColumns.map((column) => {
            const laneTasks = visibleTasksByStatus.get(column.status) ?? [];
            return (
              <section key={column.status} className={TASKS_BOARD_COLUMN_CLASS}>
                <div className={TASKS_BOARD_COLUMN_HEADER_CLASS}>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <h4 className="text-sm font-bold text-notion-text">
                        {column.label}
                      </h4>
                      <span className="rounded-sm bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold text-notion-text-muted">
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
                  {showInitialLoadingState && (
                    <div className={TASKS_BOARD_EMPTY_CLASS}>Loading tasks...</div>
                  )}
                  {!showInitialLoadingState && tasks.length === 0 && (
                    <div className={TASKS_BOARD_EMPTY_CLASS}>
                      No tasks yet.
                    </div>
                  )}
                  {!showInitialLoadingState && tasks.length > 0 && laneTasks.length === 0 && (
                    <div className={TASKS_BOARD_EMPTY_CLASS}>
                      {statusFilter === "all"
                        ? formatTaskEmptyLabel(column.status)
                        : "No results."}
                    </div>
                  )}
                  {!showInitialLoadingState &&
                    laneTasks.map((task) => (
                      <SelectableListItem
                        key={task.id}
                        className={
                          task.id === selectedTask?.id
                            ? TASKS_BOARD_CARD_ACTIVE_CLASS
                            : TASKS_BOARD_CARD_IDLE_CLASS
                        }
                        active={task.id === selectedTask?.id}
                        onClick={() => onSelectTask(task.id)}
                      >
                        <span className={TASKS_BOARD_CARD_SELECT_BUTTON_CLASS}>
                          <span className={`${TEAM_LIST_ITEM_TITLE_CLASS} font-bold`}>{task.title}</span>
                          <span className={TASKS_BOARD_CARD_META_ROW_CLASS}>
                            <span>{`upd ${formatTs(task.updated_at)}`}</span>
                            <span>{`cre ${formatTs(task.created_at)}`}</span>
                          </span>
                          <span className={TEAM_LIST_ITEM_META_CLASS}>
                            {`owner ${resolveTaskAssigneeLabel(task, assigneeLabelById)}`}
                          </span>
                          {developerMode && (
                            <span className={TEAM_LIST_ITEM_META_CLASS}>{task.id}</span>
                          )}
                        </span>
                      </SelectableListItem>
                    ))}
                </div>
              </section>
            );
          })}
        </div>
      </div>
    </div>
  );

  const detailPanel = (
    <div className={TASKS_DETAIL_PANEL_CLASS}>
      {!selectedTask && (
        <p className={TEAM_MUTED_TEXT_CLASS}>
          Select a task to inspect.
        </p>
      )}

      {selectedTask && (
        <>
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              {compactMode && (
                <p className="text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">
                  Task detail
                </p>
              )}
              <h4 className="text-[18px] font-bold text-notion-text">
                {selectedTask.title}
              </h4>
              <p className="mt-2 text-[14px] leading-relaxed text-notion-text-muted">
                Task board summary. The leader and Team runtime manage lifecycle.
              </p>
            </div>
            <div className="flex items-center gap-2">
              <StatusBadge
                label={selectedTask.status}
                tone={resolveTaskStatusTone(selectedTask.status)}
                className="team-status"
                title={`task status: ${selectedTask.status}`}
              />
              {compactMode && (
                <CloseButton
                  aria-label="Back to Kanban"
                  title="Back to Kanban"
                  onClick={() => setCompactDetailOpen(false)}
                />
              )}
            </div>
          </div>

          <div className={TASKS_DETAIL_META_CLASS}>
            <div className={TASKS_DETAIL_META_ITEM_CLASS}>
              <strong className="text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">Task</strong>
              <div className="mono mt-1 text-[12px] font-medium text-notion-text">{selectedTask.id}</div>
            </div>
            <div className={TASKS_DETAIL_META_ITEM_CLASS}>
              <strong className="text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">Created</strong>
              <div className="mt-1 text-[12px] font-medium text-notion-text">{formatTs(selectedTask.created_at)}</div>
            </div>
            <div className={TASKS_DETAIL_META_ITEM_CLASS}>
              <strong className="text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">Assignee</strong>
              <div className="mt-1 text-[12px] font-medium text-notion-text">
                {resolveTaskAssigneeLabel(selectedTask, assigneeLabelById)}
              </div>
            </div>
          </div>

          <div className="mt-2 rounded-md bg-notion-sidebar/30 px-3 py-2 text-[13px] text-notion-text-muted italic border border-notion-border/50">
            Task managed through Team runtime controls.
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <ActionButton tone="secondary" size="md" onClick={() => onOpenConversation(selectedTask.id)}>
              Open thread
            </ActionButton>
          </div>

          <div className={TASKS_RUN_LIST_CLASS}>
            <div className={TASKS_RUN_CARD_CLASS}>
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-bold text-notion-text">Latest run</p>
                  <p className="mt-1 text-[13px] leading-relaxed text-notion-text-muted">
                    {latestRun
                      ? latestRun.summary?.trim() ||
                        (latestRun.status === "completed"
                          ? "Completed."
                          : "Execution in progress.")
                      : "No run recorded yet."}
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
                  <div className="mt-4 grid gap-2 text-sm sm:grid-cols-2 lg:grid-cols-4">
                    <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                      <strong className="text-[9px] font-bold uppercase tracking-widest text-notion-text-muted">Run</strong>
                      <div className="mono mt-1 text-[11px] text-notion-text">{latestRun.id}</div>
                    </div>
                    <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                      <strong className="text-[9px] font-bold uppercase tracking-widest text-notion-text-muted">Context</strong>
                      <div className="mono mt-1 text-[11px] text-notion-text">
                        {latestRun.context_id}
                      </div>
                    </div>
                    <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                      <strong className="text-[9px] font-bold uppercase tracking-widest text-notion-text-muted">Started</strong>
                      <div className="mt-1 text-[11px] text-notion-text">
                        {formatTs(latestRun.started_at ?? latestRun.created_at)}
                      </div>
                    </div>
                    <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                      <strong className="text-[9px] font-bold uppercase tracking-widest text-notion-text-muted">Ended</strong>
                      <div className="mt-1 text-[11px] text-notion-text">
                        {latestRun.ended_at ? formatTs(latestRun.ended_at) : "running"}
                      </div>
                    </div>
                  </div>
                  <div className="mt-4 flex flex-wrap items-center gap-2">
                    <ActionButton tone="secondary" size="md" onClick={() => onOpenRun(latestRun.id)}>
                      Open Run
                    </ActionButton>
                  </div>
                </>
              )}
            </div>

            {relatedRuns.length > 1 && (
              <div className={TASKS_RUN_CARD_CLASS}>
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <p className="text-sm font-bold text-notion-text">Previous runs</p>
                  </div>
                  <span className="rounded-sm bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold text-notion-text-muted">
                    {relatedRuns.length - 1}
                  </span>
                </div>
                <div className="mt-4 space-y-2">
                  {relatedRuns.slice(1, 4).map((run) => (
                    <SelectableListItem
                      key={run.id}
                      className="flex w-full flex-col items-start gap-1 rounded-md border border-notion-border bg-white px-3 py-2 text-left transition hover:bg-notion-hover"
                      onClick={() => onOpenRun(run.id)}
                    >
                      <div className="flex w-full flex-wrap items-center justify-between gap-2">
                        <span className="mono text-[11px] text-notion-text-muted font-medium">{run.id}</span>
                        <StatusBadge
                          label={run.status}
                          tone={resolveRunStatusTone(run.status)}
                          className="team-status"
                          title={`run status: ${run.status}`}
                        />
                      </div>
                      <span className="text-[13px] font-medium text-notion-text">
                        {run.summary?.trim() || "No summary recorded."}
                      </span>
                    </SelectableListItem>
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
                  <p className="text-sm font-bold text-notion-text">Developer tools</p>
                  <p className={TASKS_DEBUG_SUMMARY_META_CLASS}>
                    Raw task context and preview.
                  </p>
                </div>
                <span className="shrink-0 text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">
                  {debugToolsOpen ? "Hide" : "Show"}
                </span>
              </summary>
              <div className={TASKS_DEBUG_BODY_CLASS}>
                <div className="flex flex-wrap items-center gap-2">
                  <ActionButton
                    tone="primary"
                    size="md"
                    onClick={() => {
                      void onCompileTaskRunPreview();
                    }}
                    disabled={!canCompileTask}
                  >
                    Compile Preview
                  </ActionButton>
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
                  <div
                    className="space-y-4 rounded-lg border border-notion-border bg-notion-sidebar/10 p-4"
                    data-team-compile-preview="true"
                  >
                    <div className="flex flex-wrap items-center gap-2">
                      <ActionButton tone="secondary" size="md" onClick={onUseCompiledRunPayload}>
                        Use Payload in Create Run
                      </ActionButton>
                      <ActionButton
                        tone="primary"
                        size="md"
                        onClick={() => {
                          void onCreateRunFromCompiledPreview();
                        }}
                        disabled={busy === "create-run"}
                      >
                        Create Run from Preview
                      </ActionButton>
                    </div>
                    <div className="grid gap-3 text-sm text-notion-text sm:grid-cols-2">
                      <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                        <strong className="text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">Conversation</strong>
                        <div className="mono mt-1 text-[12px] text-notion-text">
                          {compiledRunPreview.conversation_id}
                        </div>
                      </div>
                      <div className={TASKS_DETAIL_META_ITEM_CLASS}>
                        <strong className="text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">Context</strong>
                        <div className="mono mt-1 text-[12px] text-notion-text">
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
                    <div className="mt-4">
                      <p className="text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">
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
  );

  return (
    <div className={`${TEAM_PANEL_CARD_CLASS} p-4`} data-team-surface="kanban">
      <ToolbarRow className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Kanban</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <ActionButton
            tone="secondary"
            size="md"
            onClick={() => {
              void onRefreshTasks();
            }}
            disabled={tasksLoading}
            title="Refresh tasks"
            aria-label="Refresh tasks"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </ActionButton>
        </div>
      </ToolbarRow>

      <div className="mt-4 flex flex-wrap items-center gap-3">
        <div className="min-w-[220px] flex-1 rounded-md border border-notion-border bg-notion-sidebar/30 px-4 py-3 text-[14px] text-notion-text-muted italic">
          Kanban is the canonical Team task surface. Human requests and clarifications should go
          through <strong className="text-notion-text"># all</strong>; leader planning and Team
          runtime create and advance tasks here.
        </div>
        <ActionButton tone="secondary" size="md" onClick={() => onOpenConversation()}>
          Open # all
        </ActionButton>
      </div>

      <div className={`${TASKS_FILTER_BAR_CLASS} mt-4`}>
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
        {compactMode ? (
          compactDetailOpen ? (
            detailPanel
          ) : (
            boardPanel
          )
        ) : (
          <>
            {boardPanel}
            {detailPanel}
          </>
        )}
      </div>
    </div>
  );
}

export const TeamTasksPanel = React.memo(TeamTasksPanelImpl);
TeamTasksPanel.displayName = "TeamTasksPanel";
