import React from "react";
import { TeamRunSnapshotRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";

type TeamOverviewPanelProps = {
  snapshot: TeamRunSnapshotRecord | null;
  snapshotLoading: boolean;
  onRefreshSnapshot: () => Promise<void> | void;
  selectedMemberId: string;
  onOpenMailboxForMember: (memberId: string) => void;
};

const OVERVIEW_CARD_CLASS =
  "card rounded-2xl border border-slate-200/80 bg-white/85 shadow-sm backdrop-blur";
const OVERVIEW_TOOLBAR_CLASS = "toolbar mb-3 flex items-center justify-between gap-2";
const OVERVIEW_TOOLBAR_ACTIONS_CLASS = "actions flex items-center gap-2";
const OVERVIEW_SECONDARY_BUTTON_CLASS =
  "rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-900 hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60";
const OVERVIEW_META_CLASS =
  "teams-run-meta mb-3 rounded-xl border border-slate-200 bg-slate-50/70 p-3 text-sm";

export function TeamOverviewPanel(props: TeamOverviewPanelProps) {
  const {
    snapshot,
    snapshotLoading,
    onRefreshSnapshot,
    selectedMemberId,
    onOpenMailboxForMember,
  } = props;

  return (
    <div className={OVERVIEW_CARD_CLASS}>
      <div className={OVERVIEW_TOOLBAR_CLASS}>
        <h3>Team Snapshot</h3>
        <div className={OVERVIEW_TOOLBAR_ACTIONS_CLASS}>
          <button
            onClick={() => {
              void onRefreshSnapshot();
            }}
            disabled={snapshotLoading}
            className={OVERVIEW_SECONDARY_BUTTON_CLASS}
          >
            Refresh Snapshot
          </button>
        </div>
      </div>

      {!snapshot && <p className="muted">No snapshot yet.</p>}

      {snapshot && (
        <>
          <div className={OVERVIEW_META_CLASS}>
            <span>
              <strong>Leader:</strong> <code>{snapshot.leader_member_id ?? "-"}</code>
            </span>
            <span>
              <strong>Members:</strong> {snapshot.members.length}
            </span>
            <span>
              <strong>Pending Mailbox:</strong> {snapshot.mailbox.pending}
            </span>
            <span>
              <strong>Delivered:</strong> {snapshot.mailbox.delivered}
            </span>
            <span>
              <strong>Dead Letter:</strong> {snapshot.mailbox.dead_letter}
            </span>
            <span>
              <strong>Recent Events:</strong> {snapshot.latest_events.length}
            </span>
          </div>

          <div className="teams-member-list">
            {snapshot.members.map((member) => (
              <button
                key={member.member_id}
                className={
                  selectedMemberId === member.member_id
                    ? "team-item active rounded-lg border border-slate-300 bg-white px-3 py-2 text-left"
                    : "team-item rounded-lg border border-slate-200 bg-white px-3 py-2 text-left hover:border-slate-300"
                }
                onClick={() => onOpenMailboxForMember(member.member_id)}
              >
                <span className="team-name">
                  {member.member_id} ({member.role})
                </span>
                <StatusBadge
                  label={member.status}
                  tone={resolveTeamRunStatusTone(member.status)}
                  className="team-status"
                  title={`member status: ${member.status}`}
                />
                <span className="team-id mono">
                  {`model=${member.model ?? "-"} pending=${member.pending_inbox_count}`}
                </span>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
