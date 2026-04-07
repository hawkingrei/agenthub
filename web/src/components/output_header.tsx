import React from "react";
import { AgentRecord } from "../api";
import { StatusBadge, resolveAgentStatusTone } from "./status_badge";
import {
  OUTPUT_HEADER_DETAILS_ITEM_CLASS,
  OUTPUT_HEADER_DETAILS_LABEL_CLASS,
  OUTPUT_HEADER_DETAILS_LIST_CLASS,
  OUTPUT_HEADER_DETAILS_PANEL_CLASS,
  OUTPUT_HEADER_DETAILS_ROOT_CLASS,
  OUTPUT_HEADER_DETAILS_SUMMARY_CLASS,
  OUTPUT_HEADER_DETAILS_VALUE_CLASS,
  OUTPUT_HEADER_META_CLASS,
  OUTPUT_HEADER_ROOT_CLASS,
  OUTPUT_HEADER_SUBTITLE_CLASS,
  OUTPUT_HEADER_SUBTITLE_ROW_CLASS,
  OUTPUT_HEADER_TITLE_CLASS,
  OUTPUT_HEADER_TITLE_HEADING_CLASS,
  OUTPUT_HEADER_TITLE_MAIN_CLASS,
  OUTPUT_HEADER_TITLE_TEXT_CLASS,
} from "../ui/tailwind_classes";

type OutputHeaderDetailsItem = {
  label: string;
  value: string;
};

export function OutputHeaderDetails({
  items,
  summary = "Details",
}: {
  items: OutputHeaderDetailsItem[];
  summary?: string;
}) {
  if (items.length === 0) {
    return null;
  }
  return (
    <details className={OUTPUT_HEADER_DETAILS_ROOT_CLASS}>
      <summary className={OUTPUT_HEADER_DETAILS_SUMMARY_CLASS}>{summary}</summary>
      <div className={OUTPUT_HEADER_DETAILS_PANEL_CLASS}>
        <div className={OUTPUT_HEADER_DETAILS_LIST_CLASS}>
          {items.map((item) => (
            <div key={item.label} className={OUTPUT_HEADER_DETAILS_ITEM_CLASS}>
              <span className={OUTPUT_HEADER_DETAILS_LABEL_CLASS}>{item.label}</span>
              <span className={OUTPUT_HEADER_DETAILS_VALUE_CLASS}>{item.value}</span>
            </div>
          ))}
        </div>
      </div>
    </details>
  );
}

export type OutputHeaderProps = {
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
  activeSessionId: _activeSessionId,
  developerMode,
  hasAcp,
  thinkingStartTs,
  runStatus,
  modelLabel,
}: OutputHeaderProps) {
  void _activeSessionId;
  const titleText = activeAgent ? activeAgent.name : "No agent selected";
  const subtitleText = activeAgent
    ? activeAgent.workdir
    : "Select an agent to continue.";
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
  const detailsItems = developerMode
    ? [
        { label: "mode", value: activeAgent?.code_mode ? "on" : "off" },
        ...(updatedLabel ? [{ label: "updated", value: updatedLabel }] : []),
      ]
    : [];
  return (
    <div className={OUTPUT_HEADER_ROOT_CLASS}>
      <div className="flex min-w-0 flex-wrap items-start justify-between gap-2 sm:flex-nowrap">
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
            {developerMode ? <OutputHeaderDetails items={detailsItems} /> : null}
          </div>
        ) : null}
      </div>
      {!hasAcp ? (
        <div className={OUTPUT_HEADER_SUBTITLE_ROW_CLASS}>
          <span className={OUTPUT_HEADER_SUBTITLE_CLASS}>{subtitleText}</span>
        </div>
      ) : null}
    </div>
  );
});
