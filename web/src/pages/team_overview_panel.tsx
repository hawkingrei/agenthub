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

type TeamOverviewPanelProps = {
  snapshot: TeamRunSnapshotRecord | null;
  snapshotLoading: boolean;
  onRefreshSnapshot: () => Promise<void> | void;
  selectedMemberId: string;
  onOpenMailboxForMember: (memberId: string) => void;
  onEditAgentProfile?: () => void;
  onCloseAgentProfile?: () => void;
  profileOnly?: boolean;
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
    onEditAgentProfile,
    onCloseAgentProfile,
    profileOnly = false,
    displayNameByActorId = {},
    memberTargetNodeById = {},
  } = props;
  const selectedMember =
    snapshot?.members.find((member) => member.member_id === selectedMemberId) ?? null;
  const selectedMemberDisplayName = selectedMember
    ? resolveDisplayName(selectedMember.member_id, displayNameByActorId, selectedMember.member_id)
    : null;
  const selectedMemberNodeId = selectedMember
    ? memberTargetNodeById[selectedMember.member_id]?.trim() || null
    : null;

  return (
    <SurfaceCard className="teams-overview-panel flex min-h-0 flex-1 flex-col overflow-auto p-4">
      {!profileOnly && (
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
      )}

      {!profileOnly && <InsetSurface className="mb-6 bg-white shadow-sm">
        <h4 className="text-[15px] font-bold text-notion-text uppercase tracking-tight">Cold Start Playbook</h4>
        <p className="mt-1 text-[13px] leading-relaxed text-notion-text-muted">
          On each process start, roles should check unfinished TODOs.
        </p>
        <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2">
          <section className="flex flex-col gap-2 rounded-lg border border-notion-border/50 bg-notion-sidebar/5 p-3">
            <p className="text-[12px] font-bold uppercase tracking-wider text-notion-text">Coordinator startup</p>
            <ol className="flex flex-col gap-1.5 pl-4 text-[12px] leading-relaxed text-notion-text-muted [list-style-type:decimal]">
              <li>Scan workspace TODO files for unfinished planning.</li>
              <li>Resume existing plan or start from zero with human sync.</li>
              <li>Update AGENTS.md with plan and next checkpoint.</li>
              <li>Answer human questions directly.</li>
            </ol>
          </section>
          <section className="flex flex-col gap-2 rounded-lg border border-notion-border/50 bg-notion-sidebar/5 p-3">
            <p className="text-[12px] font-bold uppercase tracking-wider text-notion-text">Worker startup</p>
            <ol className="flex flex-col gap-1.5 pl-4 text-[12px] leading-relaxed text-notion-text-muted [list-style-type:decimal]">
              <li>Scan workspace TODO files for unfinished execution.</li>
              <li>Finish unfinished items first, then accept inbox work.</li>
              <li>If idle, request next task from coordinator.</li>
              <li>Send evidence to coordinator; keep planning with coordinator.</li>
            </ol>
          </section>
        </div>
      </InsetSurface>}

      {!snapshot && <EmptyState body="No snapshot yet." />}

      {snapshot && (
        <>
          {!profileOnly && <KeyValueList className="teams-overview-meta mb-8">
            <KeyValueItem
              label="Coordinator"
              value={
                <StatusPill className="mono border-notion-accent/20 text-notion-accent">
                  {snapshot.coordinator_member_id ?? "-"}
                </StatusPill>
              }
            />
            <KeyValueItem label="Members" value={snapshot.members.length} />
            <KeyValueItem label="Pending mailbox" value={snapshot.mailbox.pending} />
            <KeyValueItem label="Delivered" value={snapshot.mailbox.delivered} />
            <KeyValueItem label="Dead letter" value={snapshot.mailbox.dead_letter} />
            <KeyValueItem
              label="Reply obligations"
              value={snapshot.mailbox.open_reply_obligation_count ?? 0}
            />
            <KeyValueItem label="Recent events" value={snapshot.latest_events.length} />
          </KeyValueList>}

          {!profileOnly &&
          (snapshot.mailbox.open_reply_obligations?.length ?? 0) > 0 ? (
            <InsetSurface className="mb-6 bg-white shadow-sm">
              <h4 className="text-[12px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
                Open Reply Obligations
              </h4>
              <ul className="mt-3 flex flex-col gap-2">
                {(snapshot.mailbox.open_reply_obligations ?? []).map((obligation) => {
                  const agentLabel = resolveDisplayName(
                    obligation.agent_actor_id,
                    displayNameByActorId,
                    obligation.agent_actor_id
                  );
                  const humanLabel = resolveDisplayName(
                    obligation.human_actor_id,
                    displayNameByActorId,
                    obligation.human_actor_id
                  );
                  return (
                    <li
                      key={obligation.message_id}
                      className="rounded-lg border border-notion-border/60 bg-notion-sidebar/5 px-3 py-2 text-[12px] leading-relaxed text-notion-text"
                    >
                      <div className="font-medium">
                        {agentLabel} owes {humanLabel} a reply
                      </div>
                      <div className="text-notion-text-muted">
                        source={obligation.source_surface}
                        {obligation.conversation_id ? ` conversation=${obligation.conversation_id}` : ""}
                        {obligation.thread_root_message_id
                          ? ` thread=${obligation.thread_root_message_id}`
                          : ""}
                      </div>
                      {obligation.text_excerpt ? (
                        <div className="mt-1 whitespace-pre-wrap break-words text-notion-text-muted">
                          {obligation.text_excerpt}
                        </div>
                      ) : null}
                    </li>
                  );
                })}
              </ul>
            </InsetSurface>
          ) : null}

          {selectedMember && (
            <InsetSurface className="teams-agent-profile mb-6 bg-white shadow-sm">
              <PanelHeader
                title="Agent Profile"
                subtitle={selectedMemberDisplayName}
                actions={
                  <div className="flex flex-wrap items-center justify-end gap-2">
                    {profileOnly && onCloseAgentProfile ? (
                      <ActionButton
                        tone="secondary"
                        size="sm"
                        onClick={onCloseAgentProfile}
                        aria-label="Close agent profile"
                        title="Close agent profile"
                      >
                        <i className="bi bi-x-lg" aria-hidden="true" />
                        <span>Close</span>
                      </ActionButton>
                    ) : null}
                    {onEditAgentProfile ? (
                      <ActionButton
                        tone="secondary"
                        size="sm"
                        onClick={onEditAgentProfile}
                        aria-label="Edit agent profile"
                        title="Edit agent profile"
                      >
                        <i className="bi bi-pencil-square" aria-hidden="true" />
                        <span>Edit profile</span>
                      </ActionButton>
                    ) : null}
                  </div>
                }
              />
              <KeyValueList className="mt-4">
                <KeyValueItem
                  label="Member"
                  value={
                    <span className="break-words">
                      {selectedMemberDisplayName}{" "}
                      <span className="font-mono text-[11px] text-notion-text-muted">
                        {selectedMember.member_id}
                      </span>
                    </span>
                  }
                />
                <KeyValueItem label="Role" value={selectedMember.role} />
                <KeyValueItem label="Model" value={selectedMember.model ?? "-"} />
                <KeyValueItem
                  label="Status"
                  value={
                    <StatusBadge
                      label={selectedMember.status}
                      tone={resolveTeamRunStatusTone(selectedMember.status)}
                      className="team-status"
                      title={`member status: ${selectedMember.status}`}
                    />
                  }
                />
                <KeyValueItem label="Session" value={selectedMember.session_status ?? "-"} />
                <KeyValueItem label="Pending inbox" value={selectedMember.pending_inbox_count} />
                <KeyValueItem
                  label="Reply obligations"
                  value={selectedMember.reply_obligation_count ?? 0}
                />
                <KeyValueItem label="Machine" value={selectedMemberNodeId ?? "Machine unavailable"} />
              </KeyValueList>
              {selectedMember.description ? (
                <section className="mt-4">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
                    Description
                  </p>
                  <p className="mt-1 whitespace-pre-wrap break-words text-[13px] leading-5 text-notion-text">
                    {selectedMember.description}
                  </p>
                </section>
              ) : null}
              {selectedMember.prompt ? (
                <section className="mt-4">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
                    Prompt
                  </p>
                  <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-notion-border/60 bg-notion-sidebar/5 p-3 text-[12px] leading-5 text-notion-text">
                    {selectedMember.prompt}
                  </pre>
                </section>
              ) : null}
              {selectedMember.skills.length > 0 ? (
                <section className="mt-4">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
                    Skills
                  </p>
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {selectedMember.skills.map((skill) => (
                      <span
                        key={skill}
                        className="rounded-md border border-notion-border/60 bg-white px-2 py-1 text-[11px] font-medium text-notion-text"
                      >
                        {skill}
                      </span>
                    ))}
                  </div>
                </section>
              ) : null}
            </InsetSurface>
          )}

          {!profileOnly && <div className="teams-member-list flex flex-col gap-2">
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
                    className="min-w-0 flex-1 break-words whitespace-normal text-[13px] font-bold leading-5 text-notion-text"
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
                <span className="break-words whitespace-normal text-[11px] leading-relaxed text-notion-text-muted">
                  {`model=${member.model ?? "-"} pending=${member.pending_inbox_count} reply=${member.reply_obligation_count ?? 0} `}
                  {attachedNodeId ? (
                    <span className="inline-flex flex-wrap items-center gap-1 align-middle">
                      <a
                        href={buildWorkspaceNodePath(attachedNodeId)}
                        className="inline-flex items-center rounded-full border border-notion-border/60 bg-white px-2 py-0.5 text-[11px] font-semibold text-blue-700 underline decoration-transparent underline-offset-2 transition hover:border-blue-200 hover:bg-blue-50 hover:decoration-current"
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
          </div>}
        </>
      )}
    </SurfaceCard>
  );
}

export const TeamOverviewPanel = React.memo(TeamOverviewPanelImpl);
TeamOverviewPanel.displayName = "TeamOverviewPanel";
