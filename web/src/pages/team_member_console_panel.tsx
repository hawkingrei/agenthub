import React from "react";
import {
  buildWorkspaceNodePath,
  navigateToPath,
  shouldHandleInAppLinkClick,
} from "../app_route_selection";
import {
  AgentDiscoveryCardRecord,
  AgentEvent,
  TeamMemberSnapshot,
  TeamRunEventRecord,
  TeamRunSnapshotRecord,
  getTeamStepRuntimeHandleId,
} from "../api";
import {
  ActionButton,
  EmptyState,
  InsetSurface,
  PanelHeader,
  SurfaceCard,
  ToolbarRow,
} from "../ui/primitives";
import {
  resolveDisplayName,
} from "./team/mailbox_helpers";
import {
  TEAM_PANEL_INPUT_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_TITLE_CLASS,
} from "../ui/tailwind_classes";
import { windowTeamConversation } from "./team/team_conversation_viewport";

type TeamMemberConsolePanelProps = {
  snapshot: TeamRunSnapshotRecord | null;
  selectedMemberId: string;
  onSelectedMemberIdChange: (memberId: string) => void;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  selectedTargetNodeId?: string | null;
  displayNameByActorId?: Record<string, string>;
  memberEvents: AgentEvent[];
  memberEventsHasMore: boolean;
  memberEventsLoading: boolean;
  eventsLoading: boolean;
  oldestMemberEventId: number | null;
  displayedRunEvents: TeamRunEventRecord[];
  previewLimit: number;
  memberDiscoveryCard?: AgentDiscoveryCardRecord | null;
  memberDiscoveryCardLoading?: boolean;
  onRefresh: () => Promise<void> | void;
  onLoadOlder: () => Promise<void> | void;
  toPrettyJson: (value: unknown) => string;
  formatTs: (ts?: number | null) => string;
};

const MEMBER_CONSOLE_DETAIL_CLASS =
  "teams-step-body mono flex flex-col gap-1 rounded-xl border border-ui-border bg-ui-surface-soft/70 p-3 text-ui-xs text-ui-text-muted";
const MEMBER_CONSOLE_DETAIL_GRID_CLASS = "grid gap-2 md:grid-cols-2";
const MEMBER_CONSOLE_DETAIL_ITEM_CLASS = "rounded-lg border border-ui-border bg-ui-surface p-2";
const MEMBER_CONSOLE_DETAIL_LABEL_CLASS =
  "text-[11px] font-semibold uppercase tracking-wide text-ui-text-muted";
const MEMBER_CONSOLE_DETAIL_VALUE_CLASS =
  "mono mt-1 block min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ui-text-primary";
const MEMBER_CONSOLE_DETAIL_WRAP_VALUE_CLASS =
  "mono mt-1 block min-w-0 whitespace-pre-wrap break-words text-ui-text-primary";
const MEMBER_CONSOLE_DISCOVERY_CLASS = "mt-2 rounded-lg border border-ui-border bg-ui-surface p-2";
const MEMBER_CONSOLE_LIST_CLASS =
  "teams-event-list m-0 flex max-h-[420px] list-none flex-col gap-2 overflow-auto rounded-xl border border-ui-border bg-ui-surface-soft/60 p-3";
const MEMBER_CONSOLE_LIST_ITEM_CLASS = "rounded-lg border border-ui-border bg-ui-surface p-2";
const MEMBER_CONSOLE_EVENT_HEAD_CLASS =
  "teams-event-head mb-1 flex flex-wrap items-center gap-2 text-ui-xs text-ui-text-muted";
const MEMBER_CONSOLE_EMPTY_TEXT_CLASS = TEAM_MUTED_TEXT_CLASS;
const MEMBER_CONSOLE_PROMPT_DETAILS_CLASS = "rounded-lg border border-ui-border bg-ui-surface p-2";
const MEMBER_CONSOLE_PROMPT_SUMMARY_CLASS =
  "cursor-pointer text-[11px] font-semibold uppercase tracking-wide text-ui-text-muted";
const MEMBER_CONSOLE_EVENT_WINDOW_NOTICE_CLASS =
  "rounded-lg border border-ui-border bg-ui-surface-soft/70 px-2 py-1 text-[11px] text-ui-text-muted";
const MEMBER_CONSOLE_EVENT_TAIL_WINDOW_SIZE = 80;

type TeamMemberConsoleSelectOption = {
  memberId: string;
  label: string;
  role: string;
};

type TeamMemberConsoleMemberEventRow = {
  eventId: number;
  stream: string;
  tsLabel: string;
  message: string;
};

type TeamMemberConsoleRunEventRow = {
  eventId: number;
  eventType: string;
  tsLabel: string;
  payloadText: string;
};

type TeamMemberConsoleDetailItem = {
  label: string;
  value: string;
  wrap?: boolean;
  href?: string;
  onClick?: React.MouseEventHandler<HTMLAnchorElement>;
};

function TeamMemberConsolePanelImpl(props: TeamMemberConsolePanelProps) {
  const {
    snapshot,
    selectedMemberId,
    onSelectedMemberIdChange,
    selectedMemberSnapshot,
    selectedTargetNodeId,
    displayNameByActorId = {},
    memberEvents,
    memberEventsHasMore,
    memberEventsLoading,
    eventsLoading,
    oldestMemberEventId,
    displayedRunEvents,
    previewLimit,
    memberDiscoveryCard,
    memberDiscoveryCardLoading = false,
    onRefresh,
    onLoadOlder,
    toPrettyJson,
    formatTs,
  } = props;
  const mcpSkills = React.useMemo(() => {
    return (
      selectedMemberSnapshot?.skills.filter((skill) => {
        const normalized = skill.trim().toLowerCase();
        return (
          normalized.includes("mcp") ||
          normalized.includes("actor-mailbox") ||
          normalized.includes("actor-runtime")
        );
      }) ?? []
    );
  }, [selectedMemberSnapshot?.skills]);
  const attachedNodeId = selectedTargetNodeId?.trim() || null;
  const memberOptions = React.useMemo<TeamMemberConsoleSelectOption[]>(
    () =>
      (snapshot?.members ?? []).map((member) => ({
        memberId: member.member_id,
        label: resolveDisplayName(member.member_id, displayNameByActorId, member.member_id),
        role: member.role,
      })),
    [displayNameByActorId, snapshot?.members]
  );
  const memberEventRows = React.useMemo<TeamMemberConsoleMemberEventRow[]>(
    () =>
      memberEvents.map((event) => ({
        eventId: event.event_id,
        stream: event.stream,
        tsLabel: formatTs(event.ts),
        message: event.message,
      })),
    [formatTs, memberEvents]
  );
  const runEventRows = React.useMemo<TeamMemberConsoleRunEventRow[]>(
    () =>
      // Precompute preview rows so large member-console histories do not repeat
      // timestamp formatting and payload JSON rendering during every list render.
      displayedRunEvents.map((event) => ({
        eventId: event.event_id,
        eventType: event.event_type,
        tsLabel: formatTs(event.ts),
        payloadText: toPrettyJson(event.payload),
      })),
    [displayedRunEvents, formatTs, toPrettyJson]
  );
  const memberEventWindow = React.useMemo(
    () => windowTeamConversation(memberEventRows, true, MEMBER_CONSOLE_EVENT_TAIL_WINDOW_SIZE),
    [memberEventRows]
  );
  const memberDetailItems = React.useMemo<TeamMemberConsoleDetailItem[]>(() => {
    if (!selectedMemberSnapshot) {
      return [];
    }
    return [
      { label: "member_id", value: selectedMemberSnapshot.member_id },
      { label: "role", value: selectedMemberSnapshot.role },
      { label: "model", value: selectedMemberSnapshot.model ?? "-" },
      {
        label: "member_description",
        value: selectedMemberSnapshot.description ?? "-",
        wrap: true,
      },
      { label: "status", value: selectedMemberSnapshot.status },
      { label: "session_status", value: selectedMemberSnapshot.session_status ?? "-" },
      attachedNodeId
        ? {
            label: "attached_node",
            value: attachedNodeId,
            href: buildWorkspaceNodePath(attachedNodeId),
            onClick: (event) => {
              if (!shouldHandleInAppLinkClick(event)) {
                return;
              }
              event.preventDefault();
              navigateToPath(buildWorkspaceNodePath(attachedNodeId));
            },
          }
        : { label: "attached_node", value: "-" },
      {
        label: "runtime_handle_id",
        value: getTeamStepRuntimeHandleId(selectedMemberSnapshot.latest_step) ?? "-",
      },
      {
        label: "skills",
        value:
          selectedMemberSnapshot.skills.length > 0
            ? selectedMemberSnapshot.skills.join(", ")
            : "-",
        wrap: true,
      },
      {
        label: "mcp_skills",
        value: mcpSkills.length > 0 ? mcpSkills.join(", ") : "-",
        wrap: true,
      },
    ];
  }, [attachedNodeId, mcpSkills, selectedMemberSnapshot]);
  const discoveryCardDetailItems = React.useMemo<TeamMemberConsoleDetailItem[]>(() => {
    if (!memberDiscoveryCard) {
      return [];
    }
    return [
      { label: "card_id", value: memberDiscoveryCard.card_id },
      { label: "schema_version", value: memberDiscoveryCard.schema_version },
      { label: "description", value: memberDiscoveryCard.description, wrap: true },
      { label: "acp_provider", value: memberDiscoveryCard.runtime.acp_provider ?? "-" },
      { label: "worktree_mode", value: memberDiscoveryCard.runtime.worktree_mode },
      { label: "code_mode", value: memberDiscoveryCard.runtime.code_mode ? "true" : "false" },
      {
        label: "capability_tags",
        value:
          memberDiscoveryCard.capability_tags.length > 0
            ? memberDiscoveryCard.capability_tags.join(", ")
            : "-",
        wrap: true,
      },
    ];
  }, [memberDiscoveryCard]);

  return (
    <SurfaceCard className="p-4" data-team-panel="member-console">
      <PanelHeader
        title="Member Console"
        titleClassName={TEAM_PANEL_TITLE_CLASS}
        actions={
          <ToolbarRow className="justify-end gap-2">
            <ActionButton
              tone="secondary"
              size="sm"
              onClick={() => {
                void onRefresh();
              }}
              disabled={selectedMemberSnapshot ? memberEventsLoading : eventsLoading}
              title="Refresh member console"
              aria-label="Refresh member console"
            >
              <i className="bi bi-arrow-clockwise" aria-hidden="true" />
              <span>Refresh</span>
            </ActionButton>
            <ActionButton
              tone="secondary"
              size="sm"
              onClick={() => {
                void onLoadOlder();
              }}
              disabled={
                !selectedMemberSnapshot ||
                memberEventsLoading ||
                !memberEventsHasMore ||
                oldestMemberEventId == null
              }
            >
              Load Older
            </ActionButton>
          </ToolbarRow>
        }
      />

      <div className="form-row">
        <select
          className={TEAM_PANEL_INPUT_CLASS}
          value={selectedMemberId}
          onChange={(event) => onSelectedMemberIdChange(event.target.value)}
        >
          <option value="">Select member</option>
          {memberOptions.map((member) => (
            <option key={member.memberId} value={member.memberId}>
              {member.label} ({member.role})
            </option>
          ))}
        </select>
      </div>

      {selectedMemberSnapshot && (
        <InsetSurface className={MEMBER_CONSOLE_DETAIL_CLASS}>
          <div className={MEMBER_CONSOLE_DETAIL_GRID_CLASS}>
            {memberDetailItems.map((item) => (
              <div key={item.label} className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
                <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>{item.label}</div>
                {item.href ? (
                  <a
                    href={item.href}
                    className={`${MEMBER_CONSOLE_DETAIL_VALUE_CLASS} text-blue-700 underline decoration-transparent underline-offset-2 transition hover:decoration-current`}
                    title={`Open node detail for ${item.value}`}
                    onClick={item.onClick}
                  >
                    {item.value}
                  </a>
                ) : (
                  <span
                    className={
                      item.wrap
                        ? MEMBER_CONSOLE_DETAIL_WRAP_VALUE_CLASS
                        : MEMBER_CONSOLE_DETAIL_VALUE_CLASS
                    }
                  >
                    {item.value}
                  </span>
                )}
              </div>
            ))}
          </div>
          <details className={MEMBER_CONSOLE_PROMPT_DETAILS_CLASS}>
            <summary className={MEMBER_CONSOLE_PROMPT_SUMMARY_CLASS}>
              prompt
            </summary>
            <pre className={MEMBER_CONSOLE_DETAIL_WRAP_VALUE_CLASS}>
              {selectedMemberSnapshot.prompt ?? "-"}
            </pre>
          </details>
          <div className={MEMBER_CONSOLE_DISCOVERY_CLASS}>
            <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>a2a_discovery_card</div>
            {memberDiscoveryCardLoading && (
              <p className={`mt-1 ${TEAM_MUTED_TEXT_CLASS}`}>Loading discovery card...</p>
            )}
            {!memberDiscoveryCardLoading && memberDiscoveryCard && (
              <div className={MEMBER_CONSOLE_DETAIL_GRID_CLASS}>
                {discoveryCardDetailItems.map((item) => (
                  <div key={item.label} className={MEMBER_CONSOLE_DETAIL_ITEM_CLASS}>
                    <div className={MEMBER_CONSOLE_DETAIL_LABEL_CLASS}>{item.label}</div>
                    <span
                      className={
                        item.wrap
                          ? MEMBER_CONSOLE_DETAIL_WRAP_VALUE_CLASS
                          : MEMBER_CONSOLE_DETAIL_VALUE_CLASS
                      }
                    >
                      {item.value}
                    </span>
                  </div>
                ))}
              </div>
            )}
            {!memberDiscoveryCardLoading && !memberDiscoveryCard && (
              <p className={`mt-1 ${TEAM_MUTED_TEXT_CLASS}`}>
                Discovery card unavailable for this member.
              </p>
            )}
          </div>
        </InsetSurface>
      )}

      {!selectedMemberSnapshot && (
        <EmptyState
          className={MEMBER_CONSOLE_EMPTY_TEXT_CLASS}
          body={`Showing latest ${previewLimit} run records. Select a member for full member history.`}
        />
      )}

      {selectedMemberSnapshot && !getTeamStepRuntimeHandleId(selectedMemberSnapshot.latest_step) && (
        <EmptyState
          className={MEMBER_CONSOLE_EMPTY_TEXT_CLASS}
          body="Selected member has no associated session yet."
        />
      )}

      {selectedMemberSnapshot &&
        getTeamStepRuntimeHandleId(selectedMemberSnapshot.latest_step) &&
        memberEvents.length === 0 && (
          <EmptyState className={MEMBER_CONSOLE_EMPTY_TEXT_CLASS} body="No member events yet." />
        )}

      {!selectedMemberSnapshot && displayedRunEvents.length === 0 && (
        <EmptyState className={MEMBER_CONSOLE_EMPTY_TEXT_CLASS} body="No run records yet." />
      )}

      {selectedMemberSnapshot && (
        <ul className={MEMBER_CONSOLE_LIST_CLASS}>
          {memberEventWindow.offset > 0 && (
            <li className={MEMBER_CONSOLE_EVENT_WINDOW_NOTICE_CLASS}>
              {`Showing latest ${memberEventWindow.items.length} of ${memberEventWindow.total} loaded member events.`}
            </li>
          )}
          {memberEventWindow.items.map((event) => (
            <li key={event.eventId} className={MEMBER_CONSOLE_LIST_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_EVENT_HEAD_CLASS}>
                <span className="mono">#{event.eventId}</span>
                <span>{event.stream}</span>
                <span>{event.tsLabel}</span>
              </div>
              <pre className={TEAM_PANEL_PRE_CLASS}>{event.message}</pre>
            </li>
          ))}
        </ul>
      )}

      {!selectedMemberSnapshot && (
        <ul className={MEMBER_CONSOLE_LIST_CLASS}>
          {runEventRows.map((event) => (
            <li key={event.eventId} className={MEMBER_CONSOLE_LIST_ITEM_CLASS}>
              <div className={MEMBER_CONSOLE_EVENT_HEAD_CLASS}>
                <span className="mono">#{event.eventId}</span>
                <span>{event.eventType}</span>
                <span>{event.tsLabel}</span>
              </div>
              <pre className={TEAM_PANEL_PRE_CLASS}>{event.payloadText}</pre>
            </li>
          ))}
        </ul>
      )}
    </SurfaceCard>
  );
}

export const TeamMemberConsolePanel = React.memo(TeamMemberConsolePanelImpl);
TeamMemberConsolePanel.displayName = "TeamMemberConsolePanel";
