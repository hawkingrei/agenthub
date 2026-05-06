import React from "react";
import { api } from "../api";
import type { AgentTimeTriggerRecord } from "../api";
import {
  OUTPUT_HEADER_DETAILS_ROOT_CLASS,
  OUTPUT_HEADER_DETAILS_PANEL_CLASS,
  OUTPUT_HEADER_DETAILS_LIST_CLASS,
  OUTPUT_HEADER_DETAILS_ITEM_CLASS,
  OUTPUT_HEADER_DETAILS_LABEL_CLASS,
  OUTPUT_HEADER_DETAILS_VALUE_CLASS,
} from "../ui/tailwind_classes";

export type AgentTimeTriggersPanelProps = {
  agentId: string | null;
  authToken: string | null;
};

const ACTIVE_TRIGGER_STATUSES = new Set(["scheduled", "dispatching"]);

function formatFireAt(fireAtUnix: number): string {
  const d = new Date(fireAtUnix * 1000);
  const now = Date.now();
  const diffSec = Math.floor((d.getTime() - now) / 1000);

  if (diffSec < 0) {
    const ago = Math.abs(diffSec);
    if (ago < 60) return "just now";
    if (ago < 3600) return `${Math.floor(ago / 60)}m ago`;
    if (ago < 86400) return `${Math.floor(ago / 3600)}h ago`;
    if (ago < 2592000) return `${Math.floor(ago / 86400)}d ago`;
    return d.toLocaleDateString();
  }
  if (diffSec < 60) return "in <1 min";
  if (diffSec < 3600) return `in ${Math.floor(diffSec / 60)}m`;
  if (diffSec < 86400) return `in ${Math.floor(diffSec / 3600)}h`;
  if (diffSec < 2592000) return `in ${Math.floor(diffSec / 86400)}d`;
  return d.toLocaleDateString();
}

function formatFiredAt(firedAtUnix: number): string {
  return new Date(firedAtUnix * 1000).toLocaleString();
}

function triggerKindLabel(kind: string): string {
  return kind === "time" ? "time" : kind;
}

function resolveTriggerStatusTone(
  status: AgentTimeTriggerRecord["status"]
): string {
  switch (status) {
    case "scheduled":
      return "text-blue-600";
    case "dispatching":
      return "text-amber-600";
    case "fired":
      return "text-green-600";
    case "canceled":
      return "text-slate-400";
    default:
      return "text-slate-600";
  }
}

export const AgentTimeTriggersPanel = React.memo(
  function AgentTimeTriggersPanel({
    agentId,
    authToken,
  }: AgentTimeTriggersPanelProps) {
    const [triggers, setTriggers] = React.useState<
      AgentTimeTriggerRecord[] | null
    >(null);
    const [error, setError] = React.useState<string | null>(null);

    React.useEffect(() => {
      if (!agentId || !authToken) {
        setTriggers(null);
        setError(null);
        return;
      }
      let cancelled = false;
      (async () => {
        try {
          setError(null);
          const records = await api.listAgentTimeTriggers(
            authToken,
            agentId,
            100
          );
          if (!cancelled) setTriggers(records);
        } catch (err) {
          if (!cancelled) {
            setError(
              err instanceof Error ? err.message : "Failed to load triggers"
            );
            setTriggers(null);
          }
        }
      })();
      return () => {
        cancelled = true;
      };
    }, [agentId, authToken]);

    if (!agentId || !authToken) return null;
    if (error) {
      return (
        <details className={OUTPUT_HEADER_DETAILS_ROOT_CLASS}>
          <summary className="inline-flex cursor-pointer list-none items-center rounded-md border border-transparent bg-transparent px-1 py-0.5 text-[9px] font-medium uppercase tracking-[0.14em] text-notion-text-muted/62 transition hover:bg-notion-hover/65 hover:text-notion-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-notion-accent/10">
            Triggers
          </summary>
          <div className={OUTPUT_HEADER_DETAILS_PANEL_CLASS}>
            <div className="px-2 py-1.5 text-[11px] text-red-600">{error}</div>
          </div>
        </details>
      );
    }

    const loading = triggers === null;
    const activeCount = loading
      ? 0
      : triggers.filter((t) => ACTIVE_TRIGGER_STATUSES.has(t.status)).length;

    return (
      <details className={OUTPUT_HEADER_DETAILS_ROOT_CLASS}>
        <summary className="inline-flex cursor-pointer list-none items-center gap-1.5 rounded-md border border-transparent bg-transparent px-1 py-0.5 text-[9px] font-medium uppercase tracking-[0.14em] text-notion-text-muted/62 transition hover:bg-notion-hover/65 hover:text-notion-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-notion-accent/10">
          Triggers
          {activeCount > 0 && (
            <span className="inline-flex h-3.5 min-w-[14px] items-center justify-center rounded-full bg-blue-100 px-1 text-[8px] font-semibold text-blue-700">
              {activeCount}
            </span>
          )}
        </summary>
        <div className={OUTPUT_HEADER_DETAILS_PANEL_CLASS}>
          <div className={OUTPUT_HEADER_DETAILS_LIST_CLASS}>
            {loading && (
              <div className="px-2 py-1.5 text-[10px] text-notion-text-muted">
                Loading...
              </div>
            )}
            {!loading && triggers.length === 0 && (
              <div className="px-2 py-1.5 text-[10px] text-notion-text-muted">
                No scheduled triggers
              </div>
            )}
            {!loading &&
              triggers.map((trigger) => (
                <div key={trigger.id} className={OUTPUT_HEADER_DETAILS_ITEM_CLASS}>
                  <span className={OUTPUT_HEADER_DETAILS_LABEL_CLASS}>
                    {triggerKindLabel(trigger.kind)}
                  </span>
                  <span className={OUTPUT_HEADER_DETAILS_VALUE_CLASS}>
                    <span className={trigger.last_error ? "text-red-600" : resolveTriggerStatusTone(trigger.status)}>
                      {trigger.status}
                    </span>{" "}
                    · {formatFireAt(trigger.fire_at)}
                    {trigger.fired_at && trigger.status === "fired" && (
                      <>
                        <br />
                        <span className="text-green-700">
                          fired {formatFiredAt(trigger.fired_at)}
                        </span>
                      </>
                    )}
                    {trigger.last_error && (
                      <>
                        <br />
                        <span className="text-red-600">Error: {trigger.last_error}</span>
                      </>
                    )}
                    {trigger.message_text && (
                      <>
                        <br />
                        <span className="opacity-70">
                          {trigger.message_text.length > 80
                            ? trigger.message_text.slice(0, 80) + "…"
                            : trigger.message_text}
                        </span>
                      </>
                    )}
                  </span>
                </div>
              ))}
          </div>
        </div>
      </details>
    );
  }
);
AgentTimeTriggersPanel.displayName = "AgentTimeTriggersPanel";
