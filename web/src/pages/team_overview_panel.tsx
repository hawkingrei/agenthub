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

export function TeamOverviewPanel(props: TeamOverviewPanelProps) {
  const {
    snapshot,
    snapshotLoading,
    onRefreshSnapshot,
    selectedMemberId,
    onOpenMailboxForMember,
  } = props;

  return (
    <div className="card">
      <div className="toolbar">
        <h3>Team Snapshot</h3>
        <div className="actions">
          <button
            onClick={() => {
              void onRefreshSnapshot();
            }}
            disabled={snapshotLoading}
          >
            Refresh Snapshot
          </button>
        </div>
      </div>

      {!snapshot && <p className="muted">No snapshot yet.</p>}

      {snapshot && (
        <>
          <div className="teams-run-meta">
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
                className={selectedMemberId === member.member_id ? "team-item active" : "team-item"}
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
