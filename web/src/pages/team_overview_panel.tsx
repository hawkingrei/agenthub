import React from "react";
import {
  buildWorkspaceNodePath,
  navigateToPath,
  shouldHandleInAppLinkClick,
} from "../app_route_selection";
import { TeamRunSnapshotRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  ActionButton,
  EmptyState,
  InsetSurface,
  KeyValueItem,
  KeyValueList,
  PanelHeader,
  SelectableListItem,
  StatusPill,
  SurfaceCard,
} from "../ui/primitives";
import { resolveDisplayName } from "./team/mailbox_helpers";
import {
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  OVERVIEW_META_CLASS,
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
  memberTargetNodeById?: Record<string, string | null>;
};

function isMainNode(nodeId: string): boolean {
  return nodeId.trim().toLowerCase() === "main";
}

function TeamOverviewPanelImpl(props: TeamOverviewPanelProps) {
  const {
    snapshot,
    snapshotLoading,
    onRefreshSnapshot,
    selectedMemberId,
    onOpenMailboxForMember,
    displayNameByActorId = {},
    memberTargetNodeById = {},
  } = props;

  return (
    <SurfaceCard className="p-4">
      <PanelHeader
        title="Team Snapshot"
        actions={
          <ActionButton
            tone="secondary"
            size="md"
            onClick={() => {
              void onRefreshSnapshot();
            }}
            disabled={snapshotLoading}
            title="Refresh snapshot"
            aria-label="Refresh snapshot"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
          </ActionButton>
        }
      />

      <InsetSurface className="mb-6 bg-white shadow-sm">
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
      </InsetSurface>

      {!snapshot && <EmptyState className={TEAM_MUTED_TEXT_CLASS} body="No snapshot yet." />}

      {snapshot && (
        <>
          <KeyValueList className={`teams-overview-meta ${OVERVIEW_META_CLASS}`}>
            <KeyValueItem
              label="Leader"
              value={
                <StatusPill className="mono border-notion-accent/20 text-notion-accent">
                  {snapshot.leader_member_id ?? "-"}
                </StatusPill>
              }
            />
            <KeyValueItem label="Members" value={snapshot.members.length} />
            <KeyValueItem label="Pending mailbox" value={snapshot.mailbox.pending} />
            <KeyValueItem label="Delivered" value={snapshot.mailbox.delivered} />
            <KeyValueItem label="Dead letter" value={snapshot.mailbox.dead_letter} />
            <KeyValueItem label="Recent events" value={snapshot.latest_events.length} />
          </KeyValueList>

          <div className={OVERVIEW_MEMBER_LIST_CLASS}>
            {snapshot.members.map((member) => {
              const attachedNodeId = memberTargetNodeById[member.member_id]?.trim() || null;
              const attachedNodeIsMain = attachedNodeId ? isMainNode(attachedNodeId) : false;
              return (
                <SelectableListItem
                  key={member.member_id}
                  className="team-member-row"
                  active={selectedMemberId === member.member_id}
                  onClick={() => onOpenMailboxForMember(member.member_id)}
                >
                <div className="flex w-full min-w-0 items-start justify-between gap-2">
                  <span
                    className={`${TEAM_LIST_ITEM_TITLE_CLASS} min-w-0 flex-1 break-words whitespace-normal font-bold leading-5`}
                  >
                    {resolveDisplayName(member.member_id, displayNameByActorId, member.member_id)} (
                    {member.role})
                  </span>
                  <StatusBadge
                    label={member.status}
                    tone={resolveTeamRunStatusTone(member.status)}
                    className="team-status shrink-0"
                    title={`member status: ${member.status}`}
                  />
                </div>
                <span className={`${TEAM_LIST_ITEM_META_CLASS} break-words whitespace-normal`}>
                  {`model=${member.model ?? "-"} pending=${member.pending_inbox_count} `}
                  {attachedNodeId ? (
                    <span className="inline-flex flex-wrap items-center gap-1 align-middle">
                      <a
                        href={buildWorkspaceNodePath(attachedNodeId)}
                        className="inline-flex items-center rounded-full border border-ui-border bg-ui-surface px-2 py-0.5 text-[11px] font-semibold text-blue-700 underline decoration-transparent underline-offset-2 transition hover:border-blue-200 hover:bg-blue-50 hover:decoration-current"
                        title={`Open node detail for ${attachedNodeId}`}
                        onClick={(event) => {
                          if (!shouldHandleInAppLinkClick(event)) {
                            return;
                          }
                          event.preventDefault();
                          event.stopPropagation();
                          navigateToPath(buildWorkspaceNodePath(attachedNodeId));
                        }}
                      >
                        {`Machine ${attachedNodeId}`}
                      </a>
                      <span
                        className={
                          attachedNodeIsMain
                            ? "inline-flex items-center rounded-full border border-emerald-200 bg-emerald-50 px-2 py-0.5 text-[11px] font-semibold uppercase tracking-[0.08em] text-emerald-700"
                            : "inline-flex items-center rounded-full border border-sky-200 bg-sky-50 px-2 py-0.5 text-[11px] font-semibold uppercase tracking-[0.08em] text-sky-700"
                        }
                      >
                        {attachedNodeIsMain ? "local" : "remote"}
                      </span>
                    </span>
                  ) : (
                    "Machine unavailable"
                  )}
                </span>
                </SelectableListItem>
              );
            })}
          </div>
        </>
      )}
    </SurfaceCard>
  );
}

export const TeamOverviewPanel = React.memo(TeamOverviewPanelImpl);
TeamOverviewPanel.displayName = "TeamOverviewPanel";
