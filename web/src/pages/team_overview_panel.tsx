import React from "react";
import { TeamRunSnapshotRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import { resolveDisplayName } from "./team/mailbox_helpers";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_LIST_ITEM_BASE_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
  OVERVIEW_META_CLASS,
  OVERVIEW_PLAYBOOK_CLASS,
  OVERVIEW_PLAYBOOK_GRID_CLASS,
  OVERVIEW_PLAYBOOK_CARD_CLASS,
  OVERVIEW_PLAYBOOK_TITLE_CLASS,
  OVERVIEW_PLAYBOOK_LIST_CLASS,
  OVERVIEW_MEMBER_LIST_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
} from "../ui/tailwind_classes";

type TeamOverviewPanelProps = {
  snapshot: TeamRunSnapshotRecord | null;
  snapshotLoading: boolean;
  onRefreshSnapshot: () => Promise<void> | void;
  selectedMemberId: string;
  onOpenMailboxForMember: (memberId: string) => void;
  displayNameByActorId?: Record<string, string>;
};

export function TeamOverviewPanel(props: TeamOverviewPanelProps) {
  const {
    snapshot,
    snapshotLoading,
    onRefreshSnapshot,
    selectedMemberId,
    onOpenMailboxForMember,
    displayNameByActorId = {},
  } = props;

  const memberButtonClassName = (isActive: boolean) =>
    `team-member-row ${TEAM_LIST_ITEM_BASE_CLASS} ${
      isActive ? "ring-1 ring-notion-accent/30 border-notion-accent/30 bg-notion-hover shadow-md" : ""
    }`;

  return (
    <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Team Snapshot</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            onClick={() => {
              void onRefreshSnapshot();
            }}
            disabled={snapshotLoading}
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            title="Refresh snapshot"
            aria-label="Refresh snapshot"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </button>
        </div>
      </div>

      <div className={OVERVIEW_PLAYBOOK_CLASS}>
        <h4 className="text-[15px] font-bold text-notion-text uppercase tracking-tight">Cold Start Playbook</h4>
        <p className={`mt-1 ${TEAM_MUTED_TEXT_CLASS}`}>
          On each process start, roles should check unfinished TODOs.
        </p>
        <div className={OVERVIEW_PLAYBOOK_GRID_CLASS}>
          <section className={OVERVIEW_PLAYBOOK_CARD_CLASS}>
            <p className={OVERVIEW_PLAYBOOK_TITLE_CLASS}>Leader startup</p>
            <ol className={OVERVIEW_PLAYBOOK_LIST_CLASS}>
              <li>Scan workspace TODO files for unfinished planning.</li>
              <li>Resume existing plan or start from zero with human sync.</li>
              <li>Update AGENTS.md with plan and next checkpoint.</li>
              <li>Answer human questions directly.</li>
            </ol>
          </section>
          <section className={OVERVIEW_PLAYBOOK_CARD_CLASS}>
            <p className={OVERVIEW_PLAYBOOK_TITLE_CLASS}>Worker startup</p>
            <ol className={OVERVIEW_PLAYBOOK_LIST_CLASS}>
              <li>Scan workspace TODO files for unfinished execution.</li>
              <li>Finish unfinished items first, then accept inbox work.</li>
              <li>If idle, request next task from leader.</li>
              <li>Send evidence to leader; keep planning with leader.</li>
            </ol>
          </section>
        </div>
      </div>

      {!snapshot && <p className={TEAM_MUTED_TEXT_CLASS}>No snapshot yet.</p>}

      {snapshot && (
        <>
          <div className={`teams-overview-meta ${OVERVIEW_META_CLASS}`}>
            <span>
              <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Leader</strong>
              <code className="mono font-bold text-notion-accent">{snapshot.leader_member_id ?? "-"}</code>
            </span>
            <span>
              <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Members</strong>
              <span className="font-bold">{snapshot.members.length}</span>
            </span>
            <span>
              <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Pending Mailbox</strong>
              <span className="font-bold">{snapshot.mailbox.pending}</span>
            </span>
            <span>
              <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Delivered</strong>
              <span className="font-bold">{snapshot.mailbox.delivered}</span>
            </span>
            <span>
              <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Dead Letter</strong>
              <span className="font-bold">{snapshot.mailbox.dead_letter}</span>
            </span>
            <span>
              <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Recent Events</strong>
              <span className="font-bold">{snapshot.latest_events.length}</span>
            </span>
          </div>

          <div className={OVERVIEW_MEMBER_LIST_CLASS}>
            {snapshot.members.map((member) => (
              <button
                key={member.member_id}
                className={memberButtonClassName(selectedMemberId === member.member_id)}
                onClick={() => onOpenMailboxForMember(member.member_id)}
              >
                <div className="flex w-full items-center justify-between gap-2">
                  <span className={`${TEAM_LIST_ITEM_TITLE_CLASS} font-bold`}>
                    {resolveDisplayName(member.member_id, displayNameByActorId, member.member_id)} (
                    {member.role})
                  </span>
                  <StatusBadge
                    label={member.status}
                    tone={resolveTeamRunStatusTone(member.status)}
                    className="team-status"
                    title={`member status: ${member.status}`}
                  />
                </div>
                <span className={`${TEAM_LIST_ITEM_META_CLASS} break-words whitespace-normal`}>
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
