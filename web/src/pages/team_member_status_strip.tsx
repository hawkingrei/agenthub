import { Box, SimpleGrid } from "@mantine/core";
import { useMemo } from "react";
import { StatusBadge, type StatusTone } from "../components/status_badge";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  EmptyState,
  InsetSurface,
  KeyValueItem,
  KeyValueList,
  StatusPill,
  SurfaceCard,
  ToolbarRow,
} from "../ui/primitives";
import {
  TEAM_MEMBER_CARD_CLASS,
  TEAM_MEMBER_NAME_CLASS,
  TEAM_MEMBER_STATUS_ROW_CLASS,
  TEAM_MEMBER_SUMMARY_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_SECTION_TITLE_CLASS,
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
    <SurfaceCard className="p-4">
      <ToolbarRow className="mb-4">
        <h3 className={TEAM_SECTION_TITLE_CLASS}>Member Status</h3>
        <Box className={TEAM_MEMBER_SUMMARY_CLASS}>
          {TEAM_MEMBER_SUMMARY_STATUSES.map((status) => (
            <StatusPill
              key={status}
              className={TEAM_MEMBER_SUMMARY_BADGE_CLASS[status]}
            >
              {status}={summary[status]}
            </StatusPill>
          ))}
        </Box>
      </ToolbarRow>
      {members.length === 0 ? (
        <EmptyState
          title="No team members"
          body="No team members found in current team spec."
          className={TEAM_MUTED_TEXT_CLASS}
        />
      ) : (
        <SimpleGrid cols={{ base: 1, sm: 2, xl: 3 }} spacing="sm">
          {members.map((member) => {
            const lifecycle = normalizeTeamMemberLifecycle(member);
            const workStatus = normalizeTeamMemberWorkStatus(member);
            return (
              <InsetSurface
                key={member.member_id}
                className={`p-0 sm:p-0 ${TEAM_MEMBER_CARD_CLASS}`}
              >
                <Box className="flex min-w-0 items-center gap-2">
                  <Box component="span" className={TEAM_MEMBER_NAME_CLASS}>
                    {member.member_id}
                  </Box>
                </Box>
                <Box className={TEAM_MEMBER_STATUS_ROW_CLASS}>
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
                </Box>
                <KeyValueList className="mono text-[10px] text-notion-text-muted opacity-70">
                  <KeyValueItem label="role" value={member.role} />
                  <KeyValueItem label="agent" value={member.agent_name ?? "-"} />
                  {member.current_work ? (
                    <KeyValueItem
                      label="current"
                      value={member.current_work}
                      title={member.current_work}
                    />
                  ) : null}
                </KeyValueList>
              </InsetSurface>
            );
          })}
        </SimpleGrid>
      )}
    </SurfaceCard>
  );
}
