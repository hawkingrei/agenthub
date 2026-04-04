import React from "react";
import { ConnectionBadge } from "../connection_status";

type WorkbenchConnectionBadgeProps = {
  badge: ConnectionBadge;
  className: string;
};

const CONNECTION_DOT_TONE_CLASS: Record<ConnectionBadge["tone"], string> = {
  ok: "bg-emerald-600",
  warn: "bg-amber-500",
  bad: "bg-rose-500",
  muted: "bg-slate-400",
};

export function WorkbenchConnectionBadge({
  badge,
  className,
}: WorkbenchConnectionBadgeProps) {
  return (
    <div
      className={`${className} ${badge.tone}`}
      title={badge.title}
      role="status"
      aria-live="polite"
    >
      <span
        className={`session-connection-dot inline-block h-2 w-2 shrink-0 rounded-full ${CONNECTION_DOT_TONE_CLASS[badge.tone]}`}
        aria-hidden="true"
      />
      <span>{badge.label}</span>
    </div>
  );
}
