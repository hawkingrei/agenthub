import React from "react";
import { TeamRunSnapshotRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_LIST_ITEM_BASE_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
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
  "teams-overview-meta mb-3 grid min-w-0 gap-2 rounded-xl border border-slate-200 bg-slate-50/70 p-3 text-sm text-slate-700 sm:grid-cols-2 xl:grid-cols-3";
const OVERVIEW_PLAYBOOK_CLASS =
  "teams-overview-playbook mb-3 rounded-xl border border-slate-200 bg-white p-3";
const OVERVIEW_PLAYBOOK_GRID_CLASS = "grid gap-3 md:grid-cols-2";
const OVERVIEW_PLAYBOOK_CARD_CLASS = "rounded-lg border border-slate-200 bg-slate-50/70 p-3";
const OVERVIEW_PLAYBOOK_TITLE_CLASS = "text-xs font-semibold uppercase tracking-wide text-slate-500";
const OVERVIEW_PLAYBOOK_LIST_CLASS = "mt-2 list-decimal space-y-1 pl-5 text-sm text-slate-700";
const OVERVIEW_MEMBER_LIST_CLASS = "teams-member-list flex flex-col gap-2";
const OVERVIEW_MEMBER_BUTTON_BASE_CLASS =
  `team-member-row ${TEAM_LIST_ITEM_BASE_CLASS} border-slate-200`;
const OVERVIEW_MEMBER_BUTTON_ACTIVE_CLASS =
  `${OVERVIEW_MEMBER_BUTTON_BASE_CLASS} border-slate-300 ring-1 ring-slate-200`;
const OVERVIEW_MEMBER_BUTTON_IDLE_CLASS =
  `${OVERVIEW_MEMBER_BUTTON_BASE_CLASS} hover:border-slate-300`;

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
        <h4 className="text-sm font-semibold text-slate-900">Cold Start Playbook</h4>
        <p className="muted mt-1 text-sm text-slate-600">
          On each process start, both roles should check unfinished TODO items before consuming new
          mailbox tasks.
        </p>
        <div className={`${OVERVIEW_PLAYBOOK_GRID_CLASS} mt-3`}>
          <section className={OVERVIEW_PLAYBOOK_CARD_CLASS}>
            <p className={OVERVIEW_PLAYBOOK_TITLE_CLASS}>Leader startup</p>
            <ol className={OVERVIEW_PLAYBOOK_LIST_CLASS}>
              <li>Scan workspace TODO files for unfinished planning work.</li>
              <li>Resume existing plan if found; otherwise start from zero with human goal sync.</li>
              <li>Update AGENTS.md with plan, owners, and next checkpoint.</li>
              <li>Answer human planning questions directly instead of redirecting to workers.</li>
            </ol>
          </section>
          <section className={OVERVIEW_PLAYBOOK_CARD_CLASS}>
            <p className={OVERVIEW_PLAYBOOK_TITLE_CLASS}>Worker startup</p>
            <ol className={OVERVIEW_PLAYBOOK_LIST_CLASS}>
              <li>Scan workspace TODO files for unfinished execution items.</li>
              <li>Finish unfinished worker items first, then pull inbox assignments.</li>
              <li>If no assignment exists, report idle and request next task from leader.</li>
              <li>Send execution evidence to leader; keep human-facing planning with leader.</li>
            </ol>
          </section>
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
                    ? OVERVIEW_MEMBER_BUTTON_ACTIVE_CLASS
                    : OVERVIEW_MEMBER_BUTTON_IDLE_CLASS
                }
                onClick={() => onOpenMailboxForMember(member.member_id)}
              >
                <span className={TEAM_LIST_ITEM_TITLE_CLASS}>
                  {member.member_id} ({member.role})
                </span>
                <StatusBadge
                  label={member.status}
                  tone={resolveTeamRunStatusTone(member.status)}
                  className="team-status"
                  title={`member status: ${member.status}`}
                />
                <span className={TEAM_LIST_ITEM_META_CLASS}>
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
