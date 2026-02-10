import React from "react";
import { AgentRecord } from "../api";
import { isAgentActiveStatus } from "../agent_ws";
import { formatAgentModelLabel } from "../agent_presets";

type AgentsPanelProps = {
  agents: AgentRecord[];
  activeAgent: string | null;
  agentsCollapsed: boolean;
  onCollapse: () => void;
  onExpand: () => void;
  onCreateAgent: () => void;
  onSelectAgent: (id: string) => void;
  onToggleCodeMode: (id: string, next: boolean) => void;
  onStartAgent: (id: string) => void;
  onStopAgent: (id: string) => void;
  onDeleteAgent: (id: string) => void;
};

export function AgentsPanel({
  agents,
  activeAgent,
  agentsCollapsed,
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
        className={agentsCollapsed ? "workspace-left collapsed" : "workspace-left"}
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
            <div className="toolbar">
              <h2>Agents</h2>
              <div className="toolbar-actions">
                <button onClick={onCreateAgent}>Create Agent</button>
              </div>
            </div>
            <div className="agent-layout">
              <div className="agent-list">
                {agents.map((agent) => {
                  const modelLabel = formatAgentModelLabel(
                    agent.command,
                    agent.args
                  );
                  return (
                    <div
                      key={agent.id}
                      className={
                        activeAgent === agent.id ? "agent-row active" : "agent-row"
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
                          <span className={`agent-status ${agent.status}`}>
                            {agent.status}
                          </span>
                          <button
                            className="icon-button small"
                            disabled={isAgentActiveStatus(agent.status)}
                            onClick={(e) => {
                              e.stopPropagation();
                              if (!isAgentActiveStatus(agent.status)) {
                                onStartAgent(agent.id);
                              }
                            }}
                            title={
                              isAgentActiveStatus(agent.status)
                                ? "Already running"
                                : "Start"
                            }
                            aria-label="Start"
                          >
                            <i className="bi bi-play-fill" aria-hidden="true" />
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
                        <span>{agent.workdir}</span>
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
}
