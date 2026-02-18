import React from "react";
import { AgentRecord } from "../api";

export type StatusTone = "neutral" | "active" | "inactive" | "warning" | "danger";

type StatusBadgeProps = {
  label: string;
  tone: StatusTone;
  className?: string;
  title?: string;
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
  const joinedClassName = className
    ? `status-badge tone-${tone} ${className}`
    : `status-badge tone-${tone}`;
  return (
    <span className={joinedClassName} title={title}>
      {label}
    </span>
  );
});
