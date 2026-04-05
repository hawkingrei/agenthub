import React from "react";
import { AgentRecord } from "../api";

export type StatusTone = "neutral" | "active" | "inactive" | "warning" | "danger";

type StatusBadgeProps = {
  label: string;
  tone: StatusTone;
  className?: string;
  title?: string;
};

const STATUS_BADGE_BASE_CLASS =
  "status-badge inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider sm:text-[11px]";

const STATUS_BADGE_TONE_CLASS: Record<StatusTone, string> = {
  neutral:
    "border-[color:var(--status-neutral-border)] bg-[color:var(--status-neutral-bg)] text-[color:var(--status-neutral-ink)]",
  active:
    "border-[color:var(--status-active-border)] bg-[color:var(--status-active-bg)] text-[color:var(--status-active-ink)]",
  inactive:
    "border-[color:var(--status-inactive-border)] bg-[color:var(--status-inactive-bg)] text-[color:var(--status-inactive-ink)]",
  warning:
    "border-[color:var(--status-warning-border)] bg-[color:var(--status-warning-bg)] text-[color:var(--status-warning-ink)]",
  danger:
    "border-[color:var(--status-danger-border)] bg-[color:var(--status-danger-bg)] text-[color:var(--status-danger-ink)]",
};

export function resolveAgentStatusTone(status: AgentRecord["status"] | string): StatusTone {
  const normalized = status.trim().toLowerCase();
  if (normalized === "running") return "active";
  if (normalized === "idle") return "warning";
  if (normalized === "failed") return "danger";
  if (normalized === "completed" || normalized === "stopped") return "inactive";
  return "neutral";
}

export function resolveTeamLifecycleStatusTone(
  lifecycle: "active" | "inactive" | "missing"
): StatusTone {
  if (lifecycle === "active") return "active";
  if (lifecycle === "inactive") return "inactive";
  return "danger";
}

export function resolveTeamRunStatusTone(status: string): StatusTone {
  const normalized = status.trim().toLowerCase();
  if (normalized === "working" || normalized === "completed") return "active";
  if (normalized === "submitted" || normalized === "input_required") return "warning";
  if (normalized === "failed") return "danger";
  if (normalized === "canceled") return "inactive";
  if (normalized === "idle") return "inactive";
  return "neutral";
}

export const StatusBadge = React.memo(function StatusBadge({
  label,
  tone,
  className,
  title,
}: StatusBadgeProps) {
  const joinedClassName = [
    STATUS_BADGE_BASE_CLASS,
    STATUS_BADGE_TONE_CLASS[tone],
    className,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={joinedClassName} title={title}>
      {label}
    </span>
  );
});
