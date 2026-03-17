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
      className={`${className} ${badge.tone}`}
      title={badge.title}
      role="status"
      aria-live="polite"
    >
      <span className="session-connection-dot" aria-hidden="true" />
      <span>{badge.label}</span>
    </div>
  );
}
