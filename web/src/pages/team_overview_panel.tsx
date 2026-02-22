import React from "react";
import { TeamRunSnapshotRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  TEAM_ITEM_ACTIVE_CLASS,
  TEAM_ITEM_BASE_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_SURFACE_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type TeamOverviewPanelProps = {
  snapshot: TeamRunSnapshotRecord | null;
  snapshotLoading: boolean;
  onRefreshSnapshot: () => Promise<void> | void;
  selectedMemberId: string;
  onOpenMailboxForMember: (memberId: string) => void;
};

const OVERVIEW_META_CLASS =
  `teams-run-meta mb-3 grid gap-2 text-sm text-slate-700 sm:grid-cols-2 xl:grid-cols-3 ${TEAM_PANEL_SURFACE_CLASS}`;
const OVERVIEW_MEMBER_LIST_CLASS = "teams-member-list flex flex-col gap-2";

export function TeamOverviewPanel(props: TeamOverviewPanelProps) {
  const {
    snapshot,
    snapshotLoading,
    onRefreshSnapshot,
    selectedMemberId,
    onOpenMailboxForMember,
  } = props;

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Team Snapshot</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            onClick={() => {
              void onRefreshSnapshot();
            }}
            disabled={snapshotLoading}
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
          >
            Refresh Snapshot
          </button>
        </div>
      </div>

      {!snapshot && <p className="muted text-sm text-slate-600">No snapshot yet.</p>}

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

          <div className={OVERVIEW_MEMBER_LIST_CLASS}>
            {snapshot.members.map((member) => (
              <button
                key={member.member_id}
                className={
                  selectedMemberId === member.member_id
                    ? TEAM_ITEM_ACTIVE_CLASS
                    : TEAM_ITEM_BASE_CLASS
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
