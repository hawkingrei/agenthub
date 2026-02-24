import React from "react";
import { AgentRecord } from "../api";
import { StatusBadge, resolveAgentStatusTone } from "./status_badge";
import {
  OUTPUT_HEADER_META_CLASS,
  OUTPUT_HEADER_PILL_CLASS,
  OUTPUT_HEADER_ROOT_CLASS,
  OUTPUT_HEADER_SUBTITLE_CLASS,
  OUTPUT_HEADER_SUBTITLE_ROW_CLASS,
  OUTPUT_HEADER_TITLE_CLASS,
  OUTPUT_HEADER_TITLE_MAIN_CLASS,
} from "../ui/tailwind_classes";

type OutputHeaderProps = {
  activeAgent: AgentRecord | null;
  activeSessionId: string | null;
  agentsCollapsed: boolean;
  hasAcp: boolean;
  thinkingStartTs: number | null;
  runStatus?: string | null;
  modelLabel?: string | null;
  onToggleAgents: () => void;
};

export const OutputHeader = React.memo(function OutputHeader({
  activeAgent,
  activeSessionId,
  agentsCollapsed,
  hasAcp,
  thinkingStartTs,
  runStatus,
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
  const mergedStatus = (
    runStatus?.trim().toLowerCase() ||
    activeAgent?.status.trim().toLowerCase() ||
    "unknown"
  );
  const mergedStatusLabel = thinkingLabel
    ? `${mergedStatus} · ${thinkingLabel}`
    : mergedStatus;
  const mergedStatusClassToken = mergedStatus.replace(/[^a-z0-9_-]+/g, "-");
  return (
    <div className={OUTPUT_HEADER_ROOT_CLASS}>
      <div className={OUTPUT_HEADER_TITLE_CLASS}>
        <button
          className="icon-button small output-agents-toggle"
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
          <div className={OUTPUT_HEADER_TITLE_MAIN_CLASS}>
            <h2>{titleText}</h2>
            {modelLabel ? (
              <span className="agent-tag">{modelLabel}</span>
            ) : null}
          </div>
        </div>
      </div>
      {activeAgent ? (
        <div className={OUTPUT_HEADER_META_CLASS}>
          <StatusBadge
            label={mergedStatusLabel}
            tone={resolveAgentStatusTone(mergedStatus)}
            className={`agent-status status-${mergedStatusClassToken}`}
            title={`status: ${mergedStatusLabel}`}
          />
          <span className={OUTPUT_HEADER_PILL_CLASS}>
            Code mode {activeAgent.code_mode ? "on" : "off"}
          </span>
          {sessionLabel && (
            <span className="output-session mono">Session {sessionLabel}</span>
          )}
          {updatedLabel && (
            <span className="output-updated">Updated {updatedLabel}</span>
          )}
        </div>
      ) : null}
      {!hasAcp ? (
        <div className={OUTPUT_HEADER_SUBTITLE_ROW_CLASS}>
          <span className={OUTPUT_HEADER_SUBTITLE_CLASS}>{subtitleText}</span>
        </div>
      ) : null}
    </div>
  );
});
