import React from "react";
import { AgentRecord } from "../api";
import { StatusBadge, resolveAgentStatusTone } from "./status_badge";

type OutputHeaderProps = {
  activeAgent: AgentRecord | null;
  activeSessionId: string | null;
  agentsCollapsed: boolean;
  hasAcp: boolean;
  thinkingStartTs: number | null;
  modelLabel?: string | null;
  onToggleAgents: () => void;
};

const OUTPUT_HEADER_CLASS =
  "output-header rounded-xl border border-slate-200/80 bg-white/80 px-3 py-2 shadow-sm";
const OUTPUT_TITLE_CLASS = "output-title flex items-center gap-2";
const OUTPUT_TITLE_MAIN_CLASS = "output-title-main flex items-center gap-2";
const OUTPUT_META_CLASS = "output-meta flex flex-wrap items-center gap-2";
const OUTPUT_PILL_CLASS =
  "output-pill rounded-full border border-slate-300 bg-white px-2 py-1 text-xs font-medium text-slate-700";
const OUTPUT_SUBTITLE_ROW_CLASS = "output-subtitle-row mt-1";
const OUTPUT_SUBTITLE_CLASS = "output-subtitle text-sm text-slate-600";

export const OutputHeader = React.memo(function OutputHeader({
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
    <div className={OUTPUT_HEADER_CLASS}>
      <div className={OUTPUT_TITLE_CLASS}>
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
          <div className={OUTPUT_TITLE_MAIN_CLASS}>
            <h2>{titleText}</h2>
            {modelLabel ? (
              <span className="agent-tag">{modelLabel}</span>
            ) : null}
          </div>
        </div>
      </div>
      {activeAgent ? (
        <div className={OUTPUT_META_CLASS}>
          <StatusBadge
            label={activeAgent.status}
            tone={resolveAgentStatusTone(activeAgent.status)}
            className={`agent-status status-${activeAgent.status}`}
            title={`status: ${activeAgent.status}`}
          />
          <span className={OUTPUT_PILL_CLASS}>
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
        <div className={OUTPUT_SUBTITLE_ROW_CLASS}>
          <span className={OUTPUT_SUBTITLE_CLASS}>{subtitleText}</span>
        </div>
      ) : null}
    </div>
  );
});
