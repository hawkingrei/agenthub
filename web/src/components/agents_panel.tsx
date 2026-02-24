import React from "react";
import { AgentRecord } from "../api";
import { isAgentActiveStatus } from "../agent_ws";
import { formatAgentModelLabel } from "../agent_presets";
import { StatusBadge, resolveAgentStatusTone } from "./status_badge";
import {
  AGENTS_CREATE_BUTTON_CLASS,
  AGENTS_PANEL_COLLAPSED_CLASS,
  AGENTS_PANEL_EXPANDED_CLASS,
  AGENTS_ROW_ACTIVE_CLASS,
  AGENTS_ROW_CLASS,
  AGENTS_TOOLBAR_ACTIONS_CLASS,
  AGENTS_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type AgentsPanelProps = {
  agents: AgentRecord[];
  activeAgent: string | null;
  agentsCollapsed: boolean;
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
        <div className="agents-backdrop" onClick={onCollapse} />
      )}
      <div
        className={agentsCollapsed ? AGENTS_PANEL_COLLAPSED_CLASS : AGENTS_PANEL_EXPANDED_CLASS}
      >
        {agentsCollapsed ? (
          <div className="agents-rail">
            <button
              className="icon-button small"
              onClick={onExpand}
              title="Show agents"
              aria-label="Show agents"
            >
              <i className="bi bi-layout-sidebar-inset" aria-hidden="true" />
            </button>
            <div className="agents-rail-metric" title="Agents">
              <span className="value">{agents.length}</span>
              <span className="label">Agents</span>
              {hasPendingPermissions ? (
                <span
                  className="agents-rail-dot"
                  role="img"
                  aria-label="Pending permissions"
                  title="Pending permissions"
                />
              ) : null}
            </div>
            <div className="agents-rail-metric running" title="Running">
              <span className="value">{runningCount}</span>
              <span className="label">Running</span>
            </div>
            <button
              className="icon-button small"
              onClick={onCreateAgent}
              title="Create agent"
              aria-label="Create agent"
            >
              <i className="bi bi-plus-lg" aria-hidden="true" />
            </button>
          </div>
        ) : (
          <>
            <div className="mode-switch">
              <a className="mode-tag active" href="/">
                Agents
              </a>
              <a className="mode-tag" href="/teams">
                Teams
              </a>
            </div>
            <div className={AGENTS_TOOLBAR_CLASS}>
              <h2>Agents</h2>
              <div className={AGENTS_TOOLBAR_ACTIONS_CLASS}>
                <button
                  className="icon-button small agents-collapse-button"
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
            <div className="agent-layout">
              <div className="agent-list">
                {agents.map((agent) => {
                  const isStarting = Boolean(startingAgentIds[agent.id]);
                  const pendingPermissionCount =
                    pendingPermissionCounts[agent.id] ?? 0;
                  const pendingPermissionLabel = `${pendingPermissionCount} pending permission${pendingPermissionCount > 1 ? "s" : ""} for ${agent.name}`;
                  const modelLabel = formatAgentModelLabel(
                    agent.command,
                    agent.args
                  );
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
                      title={`ID: ${agent.id}\nWorkdir: ${agent.workdir}\nCommand: ${agent.command}\nStatus: ${agent.status}\nCode mode: ${agent.code_mode ? "on" : "off"}`}
                    >
                      <div className="agent-row-head">
                        <div className="agent-row-title">
                          <span className="agent-name">{agent.name}</span>
                          {pendingPermissionCount > 0 ? (
                            <span
                              className="agent-permission-dot"
                              role="img"
                              aria-label={pendingPermissionLabel}
                              title={pendingPermissionLabel}
                            />
                          ) : null}
                          {modelLabel ? (
                            <span className="agent-tag">{modelLabel}</span>
                          ) : null}
                        </div>
                        <div className="agent-row-actions">
                          <button
                            className={
                              agent.code_mode
                                ? "icon-button small code-active"
                                : "icon-button small"
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
                            className={`agent-status status-${agent.status}`}
                            title={`status: ${agent.status}`}
                          />
                          <button
                            className="icon-button small"
                            disabled={isAgentActiveStatus(agent.status) || isStarting}
                            onClick={(e) => {
                              e.stopPropagation();
                              if (!isAgentActiveStatus(agent.status) && !isStarting) {
                                onStartAgent(agent.id);
                              }
                            }}
                            title={
                              isAgentActiveStatus(agent.status)
                                ? "Already running"
                                : isStarting
                                  ? "Starting..."
                                  : "Start"
                            }
                            aria-label={isStarting ? "Starting" : "Start"}
                          >
                            <i
                              className={`bi ${isStarting ? "bi-arrow-repeat animate-spin" : "bi-play-fill"}`}
                              aria-hidden="true"
                            />
                          </button>
                          {isAgentActiveStatus(agent.status) && (
                            <button
                              className="icon-button small"
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
                            className="icon-button small danger"
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
                      <div className="agent-row-meta">
                        <span className="agent-workdir">{agent.workdir}</span>
                        <span className="agent-code-mode">
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
