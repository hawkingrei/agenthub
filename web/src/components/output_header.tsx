import React from "react";
import { AgentRecord } from "../api";
import { StatusBadge, resolveAgentStatusTone } from "./status_badge";
import {
  OUTPUT_HEADER_META_CLASS,
  OUTPUT_HEADER_PILL_CLASS,
  OUTPUT_HEADER_ROOT_CLASS,
  OUTPUT_HEADER_SESSION_CLASS,
  OUTPUT_HEADER_SUBTITLE_CLASS,
  OUTPUT_HEADER_SUBTITLE_ROW_CLASS,
  OUTPUT_HEADER_TITLE_CLASS,
  OUTPUT_HEADER_TITLE_HEADING_CLASS,
  OUTPUT_HEADER_TITLE_MAIN_CLASS,
  OUTPUT_HEADER_TITLE_TEXT_CLASS,
  OUTPUT_HEADER_UPDATED_CLASS,
} from "../ui/tailwind_classes";

type OutputHeaderProps = {
  activeAgent: AgentRecord | null;
  activeSessionId: string | null;
  developerMode: boolean;
  hasAcp: boolean;
  thinkingStartTs: number | null;
  runStatus?: string | null;
  modelLabel?: string | null;
};

export const OutputHeader = React.memo(function OutputHeader({
  activeAgent,
  activeSessionId,
  developerMode,
  hasAcp,
  thinkingStartTs,
  runStatus,
  modelLabel,
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
        <div className={OUTPUT_HEADER_TITLE_TEXT_CLASS}>
          <div className={OUTPUT_HEADER_TITLE_MAIN_CLASS}>
            <h2 className={OUTPUT_HEADER_TITLE_HEADING_CLASS}>{titleText}</h2>
            {modelLabel ? (
              <span className="agent-tag hidden sm:inline-flex">{modelLabel}</span>
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
          {developerMode && sessionLabel && (
            <span className={OUTPUT_HEADER_SESSION_CLASS}>Session {sessionLabel}</span>
          )}
          {developerMode && updatedLabel && (
            <span className={OUTPUT_HEADER_UPDATED_CLASS}>Updated {updatedLabel}</span>
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
