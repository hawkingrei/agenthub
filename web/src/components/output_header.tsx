import React from "react";
import { AgentRecord } from "../api";

type OutputHeaderProps = {
  activeAgent: AgentRecord | null;
  activeSessionId: string | null;
  agentsCollapsed: boolean;
  hasAcp: boolean;
  thinkingStartTs: number | null;
  modelLabel?: string | null;
  onToggleAgents: () => void;
};

export function OutputHeader({
  activeAgent,
  activeSessionId,
  agentsCollapsed,
  hasAcp,
  thinkingStartTs,
  modelLabel,
  onToggleAgents,
}: OutputHeaderProps) {
  const titleText = activeAgent ? activeAgent.name : "No agent selected";
  const subtitleText = activeAgent
    ? activeAgent.workdir
    : "Select an agent to continue.";
  const sessionLabel = activeSessionId ? activeSessionId.slice(0, 8) : null;
  const updatedLabel = activeAgent
    ? new Date(activeAgent.updated_at * 1000).toLocaleString()
    : null;
  const thinkingLabel =
    thinkingStartTs
      ? `thinking ${Math.max(0, Math.floor(Date.now() / 1000 - thinkingStartTs))}s`
      : null;
  return (
    <div className="output-header">
      <div className="output-title">
        <button
          className="icon-button small"
          onClick={onToggleAgents}
          title={agentsCollapsed ? "Show agents" : "Hide agents"}
          aria-label={agentsCollapsed ? "Show agents" : "Hide agents"}
        >
          <i
            className={
              agentsCollapsed ? "bi bi-chevron-right" : "bi bi-chevron-left"
            }
            aria-hidden="true"
          />
        </button>
        <div className="output-title-text">
          <div className="output-title-main">
            <h2>{titleText}</h2>
            {modelLabel ? (
              <span className="agent-tag">{modelLabel}</span>
            ) : null}
          </div>
        </div>
      </div>
      {activeAgent ? (
        <div className="output-meta">
          <span className={`agent-status ${activeAgent.status}`}>
            {activeAgent.status}
          </span>
          <span className="output-pill">
            Code mode {activeAgent.code_mode ? "on" : "off"}
          </span>
          {thinkingLabel && (
            <span className="acp-thinking">{thinkingLabel}</span>
          )}
          {sessionLabel && (
            <span className="output-session mono">Session {sessionLabel}</span>
          )}
          {updatedLabel && (
            <span className="output-updated">Updated {updatedLabel}</span>
          )}
        </div>
      ) : null}
      {!hasAcp ? (
        <div className="output-subtitle-row">
          <span className="output-subtitle">{subtitleText}</span>
        </div>
      ) : null}
    </div>
  );
}
