import { AcpPlanView } from "../acp";
import {
  ACP_PLAN_INDEX_BADGE_CLASS,
  ACP_PLAN_PRIORITY_BADGE_CLASS,
  ACP_PLAN_STATUS_BADGE_CLASS,
} from "../ui/tailwind_classes";
import { summarizePlanEntries } from "./acp_plan_summary";

type AcpPlanProps = {
  plan: AcpPlanView | null;
};

export function AcpPlan({ plan }: AcpPlanProps) {
  const entries = plan?.entries ?? [];
  const summary = summarizePlanEntries(entries);
  return (
    <section className="acp-plan-view flex min-h-0 flex-1 flex-col gap-3 p-3 sm:p-4">
      <div className="rounded-xl border border-ui-border bg-ui-surface p-3 shadow-sm sm:p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-sm font-semibold text-ui-text-primary">Current Plan</h3>
          {summary.total > 0 ? (
            <span className="text-xs text-ui-text-muted">
              {summary.completed}/{summary.total} completed
            </span>
          ) : null}
        </div>
        {summary.total === 0 ? (
          <div className="mt-3 rounded-lg border border-dashed border-ui-border-strong bg-ui-surface-soft px-3 py-6 text-sm text-ui-text-muted">
            No active plan yet.
          </div>
        ) : (
          <div className="mt-3 space-y-3">
            <div className="space-y-2">
              <div className="flex flex-wrap gap-3 text-xs text-ui-text-muted">
                <span>{summary.completed} completed</span>
                <span>{summary.active} active</span>
                <span>{summary.pending} pending</span>
              </div>
              <div className="h-2 overflow-hidden rounded-full bg-ui-surface-muted">
                <span
                  className="block h-full rounded-full bg-brand-primary transition-[width]"
                  style={{ width: `${summary.ratio}%` }}
                />
              </div>
            </div>
            <ol className="m-0 list-none space-y-2 p-0">
              {entries.map((entry, index) => (
                <li
                  className={`grid grid-cols-[auto_minmax(0,1fr)_auto_auto] items-start gap-3 rounded-md border border-ui-border bg-ui-surface-soft px-2 py-1.5`}
                  key={`${index}-${entry.content}`}
                >
                  <span className={ACP_PLAN_INDEX_BADGE_CLASS}>{index + 1}</span>
                  <span className="text-sm text-ui-text-primary">{entry.content}</span>
                  {entry.priority ? (
                    <span className={ACP_PLAN_PRIORITY_BADGE_CLASS}>{entry.priority}</span>
                  ) : null}
                  {entry.status ? (
                    <span className={ACP_PLAN_STATUS_BADGE_CLASS}>{entry.status}</span>
                  ) : null}
                </li>
              ))}
            </ol>
          </div>
        )}
      </div>
    </section>
  );
}

export type { AcpPlanProps };
