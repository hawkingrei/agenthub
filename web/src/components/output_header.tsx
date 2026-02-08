import React from "react";
import { AgentRecord } from "../api";

type OutputHeaderProps = {
  activeAgent: AgentRecord | null;
  agentsCollapsed: boolean;
  onToggleAgents: () => void;
};

export function OutputHeader({
  activeAgent,
  agentsCollapsed,
  onToggleAgents,
}: OutputHeaderProps) {
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
        <h2>Output</h2>
      </div>
      {activeAgent && (
        <span className="output-subtitle">
          {activeAgent.name} · Code mode: {activeAgent.code_mode ? "on" : "off"}
        </span>
      )}
    </div>
  );
}
