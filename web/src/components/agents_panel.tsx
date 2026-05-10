import { Box } from "@mantine/core";
import React from "react";
import { AgentRecord, isAgentActiveStatus } from "../api";
import { formatAgentModelLabel } from "../agent_presets";
import { resolveAgentStatusTone } from "./status_badge";
import { ActionButton, IconButton, cx } from "../ui/primitives";
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

const AGENTS_WORKBENCH_ROW_ICON_BUTTON_CLASS =
  "h-6 w-6 rounded-md text-[12px]";
const AGENTS_WORKBENCH_ROW_ICON_BUTTON_ACTIVE_CLASS =
  "h-6 w-6 rounded-md text-[12px]";
const AGENTS_WORKBENCH_ROW_ICON_BUTTON_DANGER_CLASS =
  "h-6 w-6 rounded-md text-[12px]";
const AGENTS_WORKBENCH_ROW_HEAD_CLASS =
  "agents-workbench-row-head flex min-w-0 items-center justify-between gap-2";
const AGENTS_WORKBENCH_ROW_TITLE_CLASS =
  "agents-workbench-row-title flex min-w-0 flex-1 items-center gap-1.5";
const AGENTS_WORKBENCH_NAME_CLASS =
  "agents-workbench-name min-w-0 flex-1 truncate text-[13px] font-medium tracking-tight text-notion-text";
const AGENTS_WORKBENCH_ROW_BADGES_CLASS =
  "agents-workbench-row-badges mt-1 flex min-w-0 items-center gap-1.5 overflow-hidden text-[10px] text-notion-text-muted";
const AGENTS_WORKBENCH_PERMISSION_DOT_CLASS =
  "agents-workbench-permission-dot inline-flex h-2 w-2 shrink-0 rounded-full bg-amber-500 shadow-[0_0_0_3px_rgba(245,158,11,0.14)]";
const AGENTS_WORKBENCH_ROW_ACTIONS_CLASS =
  "agents-workbench-row-actions inline-flex shrink-0 items-center gap-0.5 opacity-100 transition md:opacity-0 md:group-hover:opacity-100 md:group-focus-within:opacity-100";
const AGENTS_WORKBENCH_STATUS_DOT_CLASS =
  "inline-flex h-1.5 w-1.5 shrink-0 rounded-full";
const AGENTS_WORKBENCH_META_SEGMENT_CLASS = "truncate";
const AGENTS_WORKBENCH_META_SEPARATOR_CLASS =
  "inline-flex h-1 w-1 shrink-0 rounded-full bg-notion-text-muted/35";
const AGENTS_WORKBENCH_RAIL_CLASS =
  "flex h-full w-full flex-col items-center justify-between gap-3 py-3 bg-notion-sidebar border-r border-notion-border";
const AGENTS_WORKBENCH_RAIL_SUMMARY_CLASS =
  "relative flex flex-col items-center gap-0.5 rounded-md px-1.5 py-1 text-notion-text-muted";
const AGENTS_WORKBENCH_RAIL_DOT_CLASS =
  "agents-rail-dot absolute right-1.5 top-1.5 inline-flex h-2 w-2 rounded-full bg-amber-500 shadow-[0_0_0_3px_rgba(245,158,11,0.14)]";
const AGENTS_WORKBENCH_LIST_CLASS = "flex min-h-0 flex-1 flex-col gap-1 overflow-auto pr-1 mt-2";
export type AgentsPanelProps = {
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
  function resolveAgentMetaParts(agent: AgentRecord, modelLabel: string | null): string[] {
    const parts = [(isAgentActiveStatus(agent.status)) ? "online" : agent.status];
    if (modelLabel) {
      parts.push(modelLabel);
    }
    if (agent.target_node_id) {
      parts.push(agent.target_node_id);
    }
    return parts;
  }

  return (
    <>
      {!agentsCollapsed && (
        <Box className={AGENTS_PANEL_BACKDROP_CLASS} onClick={onCollapse} />
      )}
      <Box
        className={
          agentsCollapsed
            ? AGENTS_PANEL_COLLAPSED_CLASS
            : compactRows
              ? `${AGENTS_PANEL_EXPANDED_CLASS} agents-panel-compact-rows`
              : AGENTS_PANEL_EXPANDED_CLASS
        }
      >
        {agentsCollapsed ? (
          <Box className={`agents-rail ${AGENTS_WORKBENCH_RAIL_CLASS}`}>
            <Box className="flex flex-col items-center gap-2">
              <IconButton
                size="md"
                tone="default"
                onClick={onExpand}
                title="Show agents"
                aria-label="Show agents"
              >
                <i className="bi bi-layout-sidebar-inset" aria-hidden="true" />
              </IconButton>
              <Box className={AGENTS_WORKBENCH_RAIL_SUMMARY_CLASS} title="Agents">
                <Box component="span" className="text-[9px] font-medium tracking-[0.01em]">
                  Agents
                </Box>
                {hasPendingPermissions ? (
                  <Box
                    component="span"
                    className={AGENTS_WORKBENCH_RAIL_DOT_CLASS}
                    role="img"
                    aria-label="Pending permissions"
                    title="Pending permissions"
                  />
                ) : null}
              </Box>
            </Box>
            <IconButton
              size="md"
              tone="active"
              onClick={onCreateAgent}
              title="Create agent"
              aria-label="Create agent"
            >
              <i className="bi bi-plus-lg" aria-hidden="true" />
            </IconButton>
          </Box>
        ) : (
          <>
            <Box className={AGENTS_TOOLBAR_CLASS}>
              <Box component="h2" className="px-2 text-[14px] font-medium tracking-tight text-notion-text-muted">Agents</Box>
              <Box className={AGENTS_TOOLBAR_ACTIONS_CLASS}>
                <IconButton
                  size="md"
                  tone="default"
                  className="hidden lg:inline-flex"
                  onClick={onCollapse}
                  title="Hide agents"
                  aria-label="Hide agents"
                >
                  <i className="bi bi-chevron-left" aria-hidden="true" />
                </IconButton>
                <ActionButton
                  tone="secondary"
                  size="sm"
                  className={AGENTS_CREATE_BUTTON_CLASS}
                  onClick={onCreateAgent}
                >
                  Create Agent
                </ActionButton>
              </Box>
            </Box>
            <Box className={AGENTS_PANEL_BODY_CLASS}>
              <Box className={AGENTS_WORKBENCH_LIST_CLASS}>
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
                    <Box
                      role="button"
                      tabIndex={0}
                      key={agent.id}
                      className={
                        activeAgent === agent.id ? AGENTS_ROW_ACTIVE_CLASS : AGENTS_ROW_CLASS
                      }
                      onClick={() => onSelectAgent(agent.id)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          onSelectAgent(agent.id);
                        }
                      }}
                      title={`ID: ${agent.id}\nWorkdir: ${agent.workdir}\nCommand: ${agent.command}\nStatus: ${agent.status}\nCode mode: ${agent.code_mode ? "on" : "off"}\nNode: ${agent.target_node_id ?? "main"}`}
                    >
                      <Box className={AGENTS_WORKBENCH_ROW_HEAD_CLASS}>
                        <Box className={AGENTS_WORKBENCH_ROW_TITLE_CLASS}>
                          <Box component="span" className={AGENTS_WORKBENCH_NAME_CLASS}>{agent.name}</Box>
                          {pendingPermissionCount > 0 ? (
                            <Box
                              component="span"
                              className={AGENTS_WORKBENCH_PERMISSION_DOT_CLASS}
                              role="img"
                              aria-label={pendingPermissionLabel}
                              title={pendingPermissionLabel}
                            />
                          ) : null}
                        </Box>
                        <Box className={AGENTS_WORKBENCH_ROW_ACTIONS_CLASS}>
                          <IconButton
                            size="sm"
                            tone={agent.code_mode ? "active" : "subtle"}
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
                          </IconButton>
                          <IconButton
                            size="sm"
                            tone="subtle"
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
                            </IconButton>
                            {(isAgentActiveStatus(agent.status)) && (
                            <IconButton
                            size="sm"
                            tone="subtle"                              className={AGENTS_WORKBENCH_ROW_ICON_BUTTON_CLASS}
                              onClick={(e) => {
                                e.stopPropagation();
                                onStopAgent(agent.id);
                              }}
                              title="Stop"
                              aria-label="Stop"
                            >
                              <i className="bi bi-stop-fill" aria-hidden="true" />
                            </IconButton>
                          )}
                          <IconButton
                            size="sm"
                            tone="danger"
                            className={AGENTS_WORKBENCH_ROW_ICON_BUTTON_DANGER_CLASS}
                            onClick={(e) => {
                              e.stopPropagation();
                              onDeleteAgent(agent.id);
                            }}
                            title="Delete"
                            aria-label="Delete"
                          >
                            <i className="bi bi-trash" aria-hidden="true" />
                          </IconButton>
                        </Box>
                      </Box>
                      <Box className={AGENTS_WORKBENCH_ROW_BADGES_CLASS}>
                        <Box
                          component="span"
                          className={cx(
                            AGENTS_WORKBENCH_STATUS_DOT_CLASS,
                            resolveAgentStatusTone(agent.status) === "active"
                              ? "bg-emerald-500"
                              : resolveAgentStatusTone(agent.status) === "warning"
                                ? "bg-amber-500"
                                : resolveAgentStatusTone(agent.status) === "danger"
                                  ? "bg-rose-500"
                                  : "bg-slate-400"
                          )}
                          aria-hidden="true"
                        />
                        {resolveAgentMetaParts(agent, modelLabel).map((part, index) => (
                          <React.Fragment key={`${agent.id}-${part}-${index}`}>
                            {index > 0 ? (
                              <Box
                                component="span"
                                className={AGENTS_WORKBENCH_META_SEPARATOR_CLASS}
                                aria-hidden="true"
                              />
                            ) : null}
                            <Box component="span" className={AGENTS_WORKBENCH_META_SEGMENT_CLASS}>
                              {part}
                            </Box>
                          </React.Fragment>
                        ))}
                        {agent.code_mode ? (
                          <Box
                            component="span"
                            className="shrink-0 text-[10px] font-medium text-notion-text-muted"
                          >
                            code
                          </Box>
                        ) : null}
                      </Box>
                    </Box>
                  );
                })}
              </Box>
            </Box>
          </>
        )}
      </Box>
    </>
  );
});
