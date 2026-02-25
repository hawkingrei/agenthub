import React, { useMemo } from "react";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import { TeamMemberAgentStatus } from "./team/member_helpers";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_TITLE_CLASS,
} from "../ui/tailwind_classes";

type TeamMemberStatusStripProps = {
  members: TeamMemberAgentStatus[];
};

type TeamMemberLifecycle = "working" | "idle" | "stopped" | "missing" | "unknown";

type TeamMemberLifecycleSummary = {
  working: number;
  idle: number;
  stopped: number;
  missing: number;
  unknown: number;
};

const TEAM_MEMBER_SUMMARY_STATUSES: TeamMemberLifecycle[] = [
  "working",
  "idle",
  "stopped",
  "missing",
];

const TEAM_MEMBER_SUMMARY_BADGE_CLASS: Record<TeamMemberLifecycle, string> = {
  working:
    "border-[color:var(--status-active-border)] bg-[color:var(--status-active-bg)] text-[color:var(--status-active-ink)]",
  idle: "border-[color:var(--status-warning-border)] bg-[color:var(--status-warning-bg)] text-[color:var(--status-warning-ink)]",
  stopped:
    "border-[color:var(--status-inactive-border)] bg-[color:var(--status-inactive-bg)] text-[color:var(--status-inactive-ink)]",
  missing:
    "border-[color:var(--status-danger-border)] bg-[color:var(--status-danger-bg)] text-[color:var(--status-danger-ink)]",
  unknown:
    "border-[color:var(--status-neutral-border)] bg-[color:var(--status-neutral-bg)] text-[color:var(--status-neutral-ink)]",
};

const TEAM_MEMBER_SUMMARY_CLASS = "mono flex flex-wrap items-center gap-2 text-ui-xs text-ui-text-muted";
const TEAM_MEMBER_CARD_CLASS =
  "flex min-w-0 flex-col gap-1 rounded-lg border border-ui-border bg-ui-surface-soft/60 px-3 py-2";
const TEAM_MEMBER_NAME_CLASS = "min-w-0 flex-1 truncate text-ui-sm font-semibold text-ui-text-primary";
const TEAM_MEMBER_META_CLASS = "mono truncate text-ui-xs text-ui-text-muted";

export function normalizeTeamMemberLifecycle(member: TeamMemberAgentStatus): TeamMemberLifecycle {
  if (member.missing_agent) {
    return "missing";
  }
  const normalized = member.status.trim().toLowerCase();
  if (normalized === "running" || normalized === "working") {
    return "working";
  }
  if (normalized === "idle") {
    return "idle";
  }
  if (
    normalized === "stopped" ||
    normalized === "completed" ||
    normalized === "failed" ||
    normalized === "exited"
  ) {
    return "stopped";
  }
  return "unknown";
}

function resolveLifecycleTone(lifecycle: TeamMemberLifecycle): StatusTone {
  if (lifecycle === "working") return "active";
  if (lifecycle === "idle") return "warning";
  if (lifecycle === "stopped") return "inactive";
  if (lifecycle === "missing") return "danger";
  return "neutral";
}

function createLifecycleSummary(members: TeamMemberAgentStatus[]): TeamMemberLifecycleSummary {
  const summary: TeamMemberLifecycleSummary = {
    working: 0,
    idle: 0,
    stopped: 0,
    missing: 0,
    unknown: 0,
  };
  for (const member of members) {
    const lifecycle = normalizeTeamMemberLifecycle(member);
    summary[lifecycle] += 1;
  }
  return summary;
}

export function TeamMemberStatusStrip({ members }: TeamMemberStatusStripProps) {
  const summary = useMemo(() => createLifecycleSummary(members), [members]);

  return (
    <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Member Status</h3>
        <div className={TEAM_MEMBER_SUMMARY_CLASS}>
          {TEAM_MEMBER_SUMMARY_STATUSES.map((status) => (
            <span
              key={status}
              className={`rounded-md border ${TEAM_MEMBER_SUMMARY_BADGE_CLASS[status]} px-2 py-1`}
            >
              {status}={summary[status]}
            </span>
          ))}
        </div>
      </div>
      {members.length === 0 ? (
        <p className={TEAM_MUTED_TEXT_CLASS}>No team members found in current team spec.</p>
      ) : (
        <div className="grid min-w-0 gap-2 sm:grid-cols-2 xl:grid-cols-3">
          {members.map((member) => {
            const lifecycle = normalizeTeamMemberLifecycle(member);
            return (
              <div
                key={member.member_id}
                className={TEAM_MEMBER_CARD_CLASS}
              >
                <div className="flex min-w-0 items-center justify-between gap-2">
                  <span className={TEAM_MEMBER_NAME_CLASS}>
                    {member.member_id}
                  </span>
                  <StatusBadge
                    label={lifecycle}
                    tone={resolveLifecycleTone(lifecycle)}
                    className="team-status"
                    title={`member status: ${member.status}`}
                  />
                </div>
                <div className={TEAM_MEMBER_META_CLASS}>
                  role={member.role} agent={member.agent_name ?? "-"}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
