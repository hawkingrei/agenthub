import React from "react";
import { ConnectionBadge } from "../connection_status";

type WorkbenchConnectionBadgeProps = {
  badge: ConnectionBadge;
  className: string;
};

export function WorkbenchConnectionBadge({
  badge,
  className,
}: WorkbenchConnectionBadgeProps) {
  return (
    <div
      className={className}
      title={badge.title}
      role="status"
      aria-live="polite"
    >
      <span className="inline-flex h-2.5 w-2.5 rounded-full bg-black/70" aria-hidden="true" />
      <span>{badge.label}</span>
    </div>
  );
}
