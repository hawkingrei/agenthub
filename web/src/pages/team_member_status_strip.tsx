import React, { useMemo } from "react";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_TITLE_CLASS,
} from "../ui/tailwind_classes";

type TeamMemberStatusStripProps = {
  members: TeamMemberLiveState[];
};

type TeamMemberLifecycle = "working" | "idle" | "stopped" | "missing" | "unknown";
type TeamMemberWorkStatus =
  | "working"
  | "pending"
  | "blocked"
  | "done"
  | "idle"
  | "no_run"
  | "unknown";

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
  "unknown",
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
const TEAM_MEMBER_STATUS_ROW_CLASS = "flex flex-wrap items-center gap-2";
const WORKING_STATUSES = new Set(["running", "working", "in_progress"]);
const PENDING_STATUSES = new Set(["submitted", "pending", "input_required", "queued", "waiting"]);
const BLOCKED_STATUSES = new Set(["failed", "blocked", "error"]);
const DONE_STATUSES = new Set(["completed", "done", "succeeded", "success"]);
const IDLE_STATUSES = new Set(["idle", "canceled", "cancelled", "stopped", "skipped"]);

function normalizeStatusValue(status: string): string {
  const normalized = status.trim().toLowerCase();
  if (!normalized || normalized === "-") {
    return "";
  }
  return normalized;
}

export function normalizeTeamMemberLifecycle(member: TeamMemberLiveState): TeamMemberLifecycle {
  if (member.lifecycle_tone === "missing") {
    return "missing";
  }
  const normalized = normalizeStatusValue(member.lifecycle_status);
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

export function normalizeTeamMemberWorkStatus(member: TeamMemberLiveState): TeamMemberWorkStatus {
  const stepStatus = normalizeStatusValue(member.step_status);
  const runStatus = normalizeStatusValue(member.run_status);
  const normalized = stepStatus || runStatus;
  if (!normalized) {
    return "no_run";
  }
  if (WORKING_STATUSES.has(normalized)) {
    return "working";
  }
  if (PENDING_STATUSES.has(normalized)) {
    return "pending";
  }
  if (BLOCKED_STATUSES.has(normalized)) {
    return "blocked";
  }
  if (DONE_STATUSES.has(normalized)) {
    return "done";
  }
  if (IDLE_STATUSES.has(normalized)) {
    return "idle";
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

function resolveWorkStatusTone(status: TeamMemberWorkStatus): StatusTone {
  if (status === "working") return "active";
  if (status === "pending") return "warning";
  if (status === "blocked") return "danger";
  if (status === "done") return "active";
  if (status === "idle") return "inactive";
  return "neutral";
}

function createLifecycleSummary(members: TeamMemberLiveState[]): TeamMemberLifecycleSummary {
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
            const workStatus = normalizeTeamMemberWorkStatus(member);
            const pendingInbox =
              member.pending_inbox_count == null ? "-" : String(member.pending_inbox_count);
            return (
              <div
                key={member.member_id}
                className={TEAM_MEMBER_CARD_CLASS}
              >
                <div className="flex min-w-0 items-center gap-2">
                  <span className={TEAM_MEMBER_NAME_CLASS}>
                    {member.member_id}
                  </span>
                </div>
                <div className={TEAM_MEMBER_STATUS_ROW_CLASS}>
                  <StatusBadge
                    label={`work:${workStatus}`}
                    tone={resolveWorkStatusTone(workStatus)}
                    className="team-status"
                    title={`work status: run=${member.run_status} step=${member.step_status}`}
                  />
                  <StatusBadge
                    label={`agent:${lifecycle}`}
                    tone={resolveLifecycleTone(lifecycle)}
                    className="team-status"
                    title={`agent status: ${member.lifecycle_status}`}
                  />
                </div>
                <div className={TEAM_MEMBER_META_CLASS}>
                  role={member.role} agent={member.agent_name ?? "-"} pending_inbox={pendingInbox}
                </div>
                <div className={TEAM_MEMBER_META_CLASS} title={member.current_work}>
                  current={member.current_work}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
