import React from "react";
import {
  formatConversationPreview,
  type ConversationItem,
} from "../../conversation";
import {
  ACP_BUBBLE_PLAN_CLASS,
  ACP_PLAN_INDEX_BADGE_CLASS,
  ACP_PLAN_PRIORITY_BADGE_CLASS,
  ACP_PLAN_STATUS_BADGE_CLASS,
} from "../../ui/tailwind_classes";

const ACP_PLAN_CARD_CLASS =
  "acp-plan-card grid gap-2.5 rounded-[10px] border border-notion-plan-border bg-notion-plan-bg p-2.5";
const ACP_PLAN_PROGRESS_CLASS = "acp-plan-progress grid gap-1.5";
const ACP_PLAN_PROGRESS_META_CLASS =
  "acp-plan-progress-meta flex flex-wrap gap-1.5 text-[11px] text-slate-500";
const ACP_PLAN_PROGRESS_BAR_CLASS =
  "acp-plan-progress-bar mt-2 h-1.5 overflow-hidden rounded-full bg-notion-plan-progress";
const ACP_PLAN_PROGRESS_BAR_FILL_CLASS =
  "block h-full rounded-full bg-gradient-to-r from-notion-plan-progress-from to-notion-plan-progress-to";
const ACP_PLAN_LIST_CLASS = "acp-plan-list mt-3 grid list-none gap-1.5 p-0";
const ACP_PLAN_CONTENT_CLASS = "acp-plan-content min-w-0 text-sm text-slate-800";

export type PlanBubbleProps = {
  msg: Extract<ConversationItem, { kind: "agent_plan" }>;
  autoCollapse: boolean;
};

export const PlanBubble = React.memo(
  function PlanBubble({ msg, autoCollapse }: PlanBubbleProps) {
    const planSummary = summarizePlan(msg.plan_entries);
    const preview = autoCollapse ? formatConversationPreview(msg.text, 88) : "";
    const summary =
      planSummary.total > 0
        ? `Plan: ${planSummary.completed}/${planSummary.total} done · ${planSummary.active} active`
        : autoCollapse
          ? preview
            ? `Plan: ${preview}`
            : "Plan (collapsed)"
          : "Plan (collapsed)";
    return (
      <div className="acp-row group relative flex w-full flex-col items-start px-3 py-1 sm:px-4">
        <div className={ACP_BUBBLE_PLAN_CLASS}>
          <details className="acp-thought-fold acp-plan-fold">
            <summary className="cursor-pointer text-sm font-bold text-notion-text">
              {summary}
            </summary>
            <div className="acp-text mt-3 text-sm text-notion-text">
              {planSummary.total > 0 ? (
                <div className={ACP_PLAN_CARD_CLASS}>
                  <div className={ACP_PLAN_PROGRESS_CLASS}>
                    <div className={ACP_PLAN_PROGRESS_META_CLASS}>
                      <span>{planSummary.completed}/{planSummary.total} completed</span>
                      <span>{planSummary.active} active</span>
                      <span>{planSummary.pending} pending</span>
                    </div>
                    <div className={ACP_PLAN_PROGRESS_BAR_CLASS}>
                      <span
                        className={ACP_PLAN_PROGRESS_BAR_FILL_CLASS}
                        style={{ width: `${planSummary.ratio}%` }}
                      />
                    </div>
                  </div>
                  <ol className={ACP_PLAN_LIST_CLASS}>
                    {msg.plan_entries?.map((entry, idx) => {
                      const status = normalizePlanEntryStatus(entry.status);
                      return (
                        <li
                          key={`${idx}-${entry.content}`}
                          className={`${resolvePlanItemClassName(status)} ${status}`}
                        >
                          <span className={ACP_PLAN_INDEX_BADGE_CLASS}>{idx + 1}</span>
                          <span className={ACP_PLAN_CONTENT_CLASS}>{entry.content}</span>
                          {entry.priority && (
                            <span className={ACP_PLAN_PRIORITY_BADGE_CLASS}>
                              {entry.priority}
                            </span>
                          )}
                          {entry.status && (
                            <span className={ACP_PLAN_STATUS_BADGE_CLASS}>
                              {entry.status}
                            </span>
                          )}
                        </li>
                      );
                    })}
                  </ol>
                </div>
              ) : (
                <pre className="overflow-auto rounded-lg border border-notion-border bg-white p-3 text-[12px] text-notion-text">
                  {msg.text}
                </pre>
              )}
            </div>
          </details>
        </div>
      </div>
    );
  },
  (prev, next) => prev.msg === next.msg && prev.autoCollapse === next.autoCollapse
);

function resolvePlanItemClassName(status: string): string {
  if (status === "completed") {
    return "acp-plan-item grid grid-cols-[auto_minmax(0,1fr)_auto_auto] items-start gap-3 rounded-md border border-emerald-100 bg-emerald-50/30 px-2 py-1.5";
  }
  if (status === "active") {
    return "acp-plan-item grid grid-cols-[auto_minmax(0,1fr)_auto_auto] items-start gap-3 rounded-md border border-notion-accent/20 bg-notion-accent-bg px-2 py-1.5";
  }
  return "acp-plan-item grid grid-cols-[auto_minmax(0,1fr)_auto_auto] items-start gap-3 rounded-md border border-notion-border bg-white px-2 py-1.5";
}

function summarizePlan(
  entries?: Array<{ status?: string }>
): { total: number; completed: number; active: number; pending: number; ratio: number } {
  const total = entries?.length ?? 0;
  if (total === 0) {
    return { total: 0, completed: 0, active: 0, pending: 0, ratio: 0 };
  }
  let completed = 0;
  let active = 0;
  for (const entry of entries ?? []) {
    const status = normalizePlanEntryStatus(entry.status);
    if (status === "completed") completed += 1;
    else if (status === "active") active += 1;
  }
  const pending = Math.max(0, total - completed - active);
  return {
    total,
    completed,
    active,
    pending,
    ratio: Math.round((completed / total) * 100),
  };
}

function normalizePlanEntryStatus(status?: string): "completed" | "active" | "pending" {
  if (!status) return "pending";
  const normalized = status.trim().toLowerCase().replace(/[\s-]+/g, "_");
  if (normalized === "completed" || normalized === "done" || normalized === "finished") {
    return "completed";
  }
  if (normalized === "in_progress" || normalized === "running" || normalized === "active") {
    return "active";
  }
  return "pending";
}
