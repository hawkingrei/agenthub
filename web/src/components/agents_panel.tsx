import React from "react";
import { AgentRecord } from "../api";
import { isAgentActiveStatus } from "../agent_ws";
import { formatAgentModelLabel } from "../agent_presets";
import { StatusBadge, resolveAgentStatusTone } from "./status_badge";
import {
  AGENTS_PANEL_BACKDROP_CLASS,
  AGENTS_PANEL_BODY_CLASS,
  AGENTS_CREATE_BUTTON_CLASS,
  AGENTS_PANEL_COLLAPSED_CLASS,
  AGENTS_PANEL_EXPANDED_CLASS,
  AGENTS_ROW_ACTIVE_CLASS,
  AGENTS_ROW_CLASS,
  AGENTS_TOOLBAR_ACTIONS_CLASS,
  AGENTS_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

const AGENTS_WORKBENCH_ICON_BUTTON_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-md border border-notion-border bg-white text-notion-text-muted shadow-sm transition hover:bg-notion-hover hover:text-notion-text active:translate-y-px sm:h-9 sm:w-9";
const AGENTS_WORKBENCH_ICON_BUTTON_ACTIVE_CLASS =
  "inline-flex h-8 w-8 items-center justify-center rounded-md border border-notion-accent/30 bg-notion-accent-bg text-notion-accent shadow-sm transition hover:bg-notion-accent/10 active:translate-y-px sm:h-9 sm:w-9";
const AGENTS_WORKBENCH_ROW_ICON_BUTTON_CLASS =
  "inline-flex h-7 w-7 items-center justify-center rounded-md text-[13px] text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text active:translate-y-px sm:h-8 sm:w-8 sm:text-[14px]";
const AGENTS_WORKBENCH_ROW_ICON_BUTTON_ACTIVE_CLASS =
  "inline-flex h-7 w-7 items-center justify-center rounded-md bg-notion-accent-bg text-[13px] text-notion-accent transition hover:bg-notion-accent/10 active:translate-y-px sm:h-8 sm:w-8 sm:text-[14px]";
const AGENTS_WORKBENCH_ROW_ICON_BUTTON_DANGER_CLASS =
  "inline-flex h-7 w-7 items-center justify-center rounded-md text-[13px] text-notion-text-muted transition hover:bg-red-50 hover:text-red-600 active:translate-y-px sm:h-8 sm:w-8 sm:text-[14px]";
const AGENTS_WORKBENCH_ROW_HEAD_CLASS =
  "agents-workbench-row-head flex min-w-0 items-start justify-between gap-2";
const AGENTS_WORKBENCH_ROW_TITLE_CLASS =
  "agents-workbench-row-title flex min-w-0 flex-1 items-start gap-1.5";
const AGENTS_WORKBENCH_NAME_CLASS =
  "agents-workbench-name min-w-0 flex-1 truncate text-[14px] font-bold tracking-tight text-notion-text";
const AGENTS_WORKBENCH_ROW_BADGES_CLASS =
  "agents-workbench-row-badges mt-1 flex min-w-0 flex-wrap items-center gap-1.5";
const AGENTS_WORKBENCH_PERMISSION_DOT_CLASS =
  "agents-workbench-permission-dot inline-flex h-2 w-2 shrink-0 rounded-full bg-amber-500 shadow-[0_0_0_3px_rgba(245,158,11,0.14)]";
const AGENTS_WORKBENCH_TAG_CLASS =
  "agents-workbench-tag inline-flex shrink-0 items-center rounded-full border border-notion-border bg-white px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted";
const AGENTS_WORKBENCH_ROW_ACTIONS_CLASS =
  "agents-workbench-row-actions inline-flex shrink-0 items-center gap-1";
const AGENTS_WORKBENCH_ROW_META_CLASS =
  "agents-workbench-row-meta mt-1.5 hidden flex-col gap-0.5 sm:flex";
const AGENTS_WORKBENCH_WORKDIR_CLASS =
  "agents-workbench-workdir truncate text-[11px] leading-relaxed text-notion-text";
const AGENTS_WORKBENCH_CODE_MODE_CLASS =
  "agents-workbench-code-mode text-[10px] font-medium uppercase tracking-wider text-notion-text-muted";
const AGENTS_WORKBENCH_RAIL_CLASS = "flex h-full w-full flex-col items-center gap-4 py-4 bg-notion-sidebar border-r border-notion-border";
const AGENTS_WORKBENCH_METRIC_CLASS =
  "relative grid gap-1 justify-items-center rounded-lg border border-notion-border bg-white px-3 py-2 text-[9px] font-bold uppercase tracking-widest text-notion-text-muted shadow-sm";
const AGENTS_WORKBENCH_RAIL_DOT_CLASS =
  "agents-rail-dot absolute right-1.5 top-1.5 inline-flex h-2 w-2 rounded-full bg-amber-500 shadow-[0_0_0_3px_rgba(245,158,11,0.14)]";
const AGENTS_WORKBENCH_LIST_CLASS = "flex min-h-0 flex-1 flex-col gap-1 overflow-auto pr-1 mt-2";
const AGENTS_WORKBENCH_TOOLBAR_TITLE_CLASS =
  "text-sm font-bold uppercase tracking-widest text-notion-text-muted px-2";

type AgentsPanelProps = {
  agents: AgentRecord[];
  activeAgent: string | null;
  agentsCollapsed: boolean;
  compactRows: boolean;
  hasPendingPermissions: boolean;
  pendingPermissionCounts: Record<string, number>;
  startingAgentIds: Record<string, boolean>;
  onCollapse: () => void;
  onExpand: () => void;
  onCreateAgent: () => void;
  onSelectAgent: (id: string) => void;
  onToggleCodeMode: (id: string, next: boolean) => void;
  onStartAgent: (id: string) => void;
  onStopAgent: (id: string) => void;
  onDeleteAgent: (id: string) => void;
};

export const AgentsPanel = React.memo(function AgentsPanel({
  agents,
  activeAgent,
  agentsCollapsed,
  compactRows,
  hasPendingPermissions,
  pendingPermissionCounts,
  startingAgentIds,
  onCollapse,
  onExpand,
  onCreateAgent,
  onSelectAgent,
  onToggleCodeMode,
  onStartAgent,
  onStopAgent,
  onDeleteAgent,
}: AgentsPanelProps) {
  const runningCount = agents.filter((agent) => agent.status === "running")
    .length;
  return (
    <>
      {!agentsCollapsed && (
        <div className={AGENTS_PANEL_BACKDROP_CLASS} onClick={onCollapse} />
      )}
      <div
        className={
          agentsCollapsed
            ? AGENTS_PANEL_COLLAPSED_CLASS
            : compactRows
              ? `${AGENTS_PANEL_EXPANDED_CLASS} agents-panel-compact-rows`
              : AGENTS_PANEL_EXPANDED_CLASS
        }
      >
        {agentsCollapsed ? (
          <div className={`agents-rail ${AGENTS_WORKBENCH_RAIL_CLASS}`}>
            <button
              className={AGENTS_WORKBENCH_ICON_BUTTON_CLASS}
              onClick={onExpand}
              title="Show agents"
              aria-label="Show agents"
            >
              <i className="bi bi-layout-sidebar-inset" aria-hidden="true" />
            </button>
            <div className={AGENTS_WORKBENCH_METRIC_CLASS} title="Agents">
              <span className="value">{agents.length}</span>
              <span className="label">Agents</span>
              {hasPendingPermissions ? (
                <span
                  className={AGENTS_WORKBENCH_RAIL_DOT_CLASS}
                  role="img"
                  aria-label="Pending permissions"
                  title="Pending permissions"
                />
              ) : null}
            </div>
            <div className={AGENTS_WORKBENCH_METRIC_CLASS} title="Running">
              <span className="value">{runningCount}</span>
              <span className="label">Running</span>
            </div>
            <button
              className={AGENTS_WORKBENCH_ICON_BUTTON_ACTIVE_CLASS}
              onClick={onCreateAgent}
              title="Create agent"
              aria-label="Create agent"
            >
              <i className="bi bi-plus-lg" aria-hidden="true" />
            </button>
          </div>
        ) : (
          <>
            <div className={AGENTS_TOOLBAR_CLASS}>
              <h2 className={AGENTS_WORKBENCH_TOOLBAR_TITLE_CLASS}>Agents</h2>
              <div className={AGENTS_TOOLBAR_ACTIONS_CLASS}>
                <button
                  className={`hidden lg:inline-flex ${AGENTS_WORKBENCH_ICON_BUTTON_CLASS}`}
                  onClick={onCollapse}
                  title="Hide agents"
                  aria-label="Hide agents"
                >
                  <i className="bi bi-chevron-left" aria-hidden="true" />
                </button>
                <button className={AGENTS_CREATE_BUTTON_CLASS} onClick={onCreateAgent}>
                  Create Agent
                </button>
              </div>
            </div>
            <div className={AGENTS_PANEL_BODY_CLASS}>
              <div className={AGENTS_WORKBENCH_LIST_CLASS}>
                {agents.map((agent) => {
                  const isStarting = Boolean(startingAgentIds[agent.id]);
                  const isRemoteTarget = Boolean(agent.target_node_id);
                  const isActive = isAgentActiveStatus(agent.status);
                  const pendingPermissionCount =
                    pendingPermissionCounts[agent.id] ?? 0;
                  const pendingPermissionLabel = `${pendingPermissionCount} pending permission${pendingPermissionCount > 1 ? "s" : ""} for ${agent.name}`;
                  const modelLabel = formatAgentModelLabel(
                    agent.command,
                    agent.args
                  );
                  const startButtonTitle = isStarting
                    ? "Starting..."
                    : isActive
                      ? "Already running"
                      : isRemoteTarget
                        ? `Start on node ${agent.target_node_id}`
                        : "Start";
                  const startButtonAriaLabel = isStarting
                    ? "Starting"
                    : isActive
                      ? "Already running"
                      : isRemoteTarget
                        ? `Start agent on node ${agent.target_node_id}`
                        : "Start";
                  return (
                    <div
                      key={agent.id}
                      className={
                        activeAgent === agent.id ? AGENTS_ROW_ACTIVE_CLASS : AGENTS_ROW_CLASS
                      }
                      role="button"
                      tabIndex={0}
                      onClick={() => onSelectAgent(agent.id)}
                      onKeyDown={(e) => {
                        if (
                          e.key === "Enter" ||
                          e.key === " " ||
                          e.key === "Spacebar"
                        ) {
                          e.preventDefault();
                          onSelectAgent(agent.id);
                        }
                      }}
                      title={`ID: ${agent.id}\nWorkdir: ${agent.workdir}\nCommand: ${agent.command}\nStatus: ${agent.status}\nCode mode: ${agent.code_mode ? "on" : "off"}\nNode: ${agent.target_node_id ?? "main"}`}
                    >
                      <div className={AGENTS_WORKBENCH_ROW_HEAD_CLASS}>
                        <div className={AGENTS_WORKBENCH_ROW_TITLE_CLASS}>
                          <span className={AGENTS_WORKBENCH_NAME_CLASS}>{agent.name}</span>
                          {pendingPermissionCount > 0 ? (
                            <span
                              className={AGENTS_WORKBENCH_PERMISSION_DOT_CLASS}
                              role="img"
                              aria-label={pendingPermissionLabel}
                              title={pendingPermissionLabel}
                            />
                          ) : null}
                        </div>
                        <div className={AGENTS_WORKBENCH_ROW_ACTIONS_CLASS}>
                          <button
                            className={
                              agent.code_mode
                                ? AGENTS_WORKBENCH_ROW_ICON_BUTTON_ACTIVE_CLASS
                                : AGENTS_WORKBENCH_ROW_ICON_BUTTON_CLASS
                            }
                            onClick={(e) => {
                              e.stopPropagation();
                              onToggleCodeMode(agent.id, !agent.code_mode);
                            }}
                            title={
                              agent.code_mode
                                ? "Disable code mode"
                                : "Enable code mode"
                            }
                            aria-label={
                              agent.code_mode
                                ? "Disable code mode"
                                : "Enable code mode"
                            }
                            aria-pressed={agent.code_mode}
                          >
                            <i className="bi bi-code-slash" aria-hidden="true" />
                          </button>
                          <StatusBadge
                            label={agent.status}
                            tone={resolveAgentStatusTone(agent.status)}
                            className={`agents-workbench-status status-${agent.status} hidden sm:inline-flex px-2 py-0.5 text-[10px]`}
                            title={`status: ${agent.status}`}
                          />
                          <button
                            className={AGENTS_WORKBENCH_ROW_ICON_BUTTON_CLASS}
                            disabled={isActive || isStarting}
                            onClick={(e) => {
                              e.stopPropagation();
                              if (!isActive && !isStarting) {
                                onStartAgent(agent.id);
                              }
                            }}
                            title={startButtonTitle}
                            aria-label={startButtonAriaLabel}
                          >
                            <i
                              className={`bi ${isStarting ? "bi-arrow-repeat animate-spin" : "bi-play-fill"}`}
                              aria-hidden="true"
                            />
                          </button>
                          {isAgentActiveStatus(agent.status) && (
                            <button
                              className={AGENTS_WORKBENCH_ROW_ICON_BUTTON_CLASS}
                              onClick={(e) => {
                                e.stopPropagation();
                                onStopAgent(agent.id);
                              }}
                              title="Stop"
                              aria-label="Stop"
                            >
                              <i className="bi bi-stop-fill" aria-hidden="true" />
                            </button>
                          )}
                          <button
                            className={AGENTS_WORKBENCH_ROW_ICON_BUTTON_DANGER_CLASS}
                            onClick={(e) => {
                              e.stopPropagation();
                              onDeleteAgent(agent.id);
                            }}
                            title="Delete"
                            aria-label="Delete"
                          >
                            <i className="bi bi-trash" aria-hidden="true" />
                          </button>
                        </div>
                      </div>
                      <div className={AGENTS_WORKBENCH_ROW_BADGES_CLASS}>
                        {modelLabel ? (
                          <span className={AGENTS_WORKBENCH_TAG_CLASS}>{modelLabel}</span>
                        ) : null}
                        {isRemoteTarget ? (
                          <span className={AGENTS_WORKBENCH_TAG_CLASS}>
                            node:{agent.target_node_id}
                          </span>
                        ) : null}
                        <StatusBadge
                          label={agent.status}
                          tone={resolveAgentStatusTone(agent.status)}
                          className={`agents-workbench-status status-${agent.status} px-2 py-0.5 text-[10px]`}
                          title={`status: ${agent.status}`}
                        />
                      </div>
                      <div className={AGENTS_WORKBENCH_ROW_META_CLASS}>
                        <span className={AGENTS_WORKBENCH_WORKDIR_CLASS}>{agent.workdir}</span>
                        <span className={AGENTS_WORKBENCH_CODE_MODE_CLASS}>
                          Code mode: {agent.code_mode ? "on" : "off"}
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </>
        )}
      </div>
    </>
  );
});
