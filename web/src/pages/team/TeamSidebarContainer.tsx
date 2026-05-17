import React from "react";
import { TeamSidebar } from "../team_sidebar";
import type { TeamDefinitionRecord } from "../../api";
import type { TeamChannelItem } from "./channel_metadata";
import type { TeamMemberLiveState } from "./member_helpers";
import type { WorkspaceLens } from "../../app_route_selection";
import type { TeamTab } from "./state";

type TeamSidebarProps = React.ComponentProps<typeof TeamSidebar>;

type TeamSidebarContainerProps = {
  isRoot: boolean;
  developerMode: boolean;
  busy: string | null;
  refreshTeams: () => void;
  openCreateTeamModal: () => void;
  newTeamName: string;
  coordinatorMemberId: string;
  selectedTeamWorkerCount: number;
  teams: TeamDefinitionRecord[];
  selectedTeam: TeamDefinitionRecord | null;
  selectedTeamId: string | null;
  selectedTeamRuntimeStatus: TeamSidebarProps["selectedTeamRuntimeStatus"];
  selectedTeamHasConfiguredMembers: boolean;
  teamMemberSummaryByTeamId: TeamSidebarProps["teamMemberSummaryByTeamId"];
  selectedTeamMemberLiveStates: TeamMemberLiveState[];
  channelItems: ReadonlyArray<TeamChannelItem>;
  routeChannelId: string;
  focusedAgentMemberId: string;
  routeWorkspaceLens: WorkspaceLens | null;
  tab: TeamTab;
  navigateToTeamDetail: (id: string) => void;
  navigateToTeamSelector: () => void;
  onSelectChannel: (id: string) => void;
  onCreateChannel: (payload: { channelId: string; description: string }) => void;
  onDeleteChannel: (id: string) => void;
  creatingChannel: boolean;
  deletingChannelId: string | null;
  onSelectKanbanSubject: () => void;
  onSelectWorkspaceLens: (value: string) => void;
  onSelectAgentWorkspace: (memberId: string, tab?: TeamTab) => void;
  onOpenTeamMemberForge: () => void;
  onOpenTeamMemberCopyExisting: () => void;
  teamMemberForgeLabel: string;
  teamMemberCopyExistingLabel: string;
  onStartTeamRuntime: () => void;
  onStopTeamRuntime: () => void;
  onOpenMachines: () => void;
  currentMachineId: string | null;
  onOpenCurrentMachine: () => void;
};

export const TeamSidebarContainer = React.memo(function TeamSidebarContainer(
  props: TeamSidebarContainerProps
) {
  const {
    isRoot,
    developerMode,
    busy,
    refreshTeams,
    openCreateTeamModal,
    newTeamName,
    coordinatorMemberId,
    selectedTeamWorkerCount,
    teams,
    selectedTeam,
    selectedTeamId,
    selectedTeamRuntimeStatus,
    selectedTeamHasConfiguredMembers,
    teamMemberSummaryByTeamId,
    selectedTeamMemberLiveStates,
    channelItems,
    routeChannelId,
    focusedAgentMemberId,
    routeWorkspaceLens,
    tab,
    navigateToTeamDetail,
    navigateToTeamSelector,
    onSelectChannel,
    onCreateChannel,
    onDeleteChannel,
    creatingChannel,
    deletingChannelId,
    onSelectKanbanSubject,
    onSelectWorkspaceLens,
    onSelectAgentWorkspace,
    onOpenTeamMemberForge,
    onOpenTeamMemberCopyExisting,
    teamMemberForgeLabel,
    teamMemberCopyExistingLabel,
    onStartTeamRuntime,
    onStopTeamRuntime,
    onOpenMachines,
    currentMachineId,
    onOpenCurrentMachine,
  } = props;

  return (
    <TeamSidebar
      showTeamSelector={false}
      isRoot={isRoot}
      developerMode={developerMode}
      busy={busy}
      onRefreshTeams={refreshTeams}
      onOpenCreateTeam={openCreateTeamModal}
      draftTeamName={newTeamName}
      coordinatorMemberId={coordinatorMemberId}
      configuredWorkerCount={selectedTeamWorkerCount}
      teams={teams}
      selectedTeam={selectedTeam}
      selectedTeamId={selectedTeamId}
      selectedTeamRuntimeStatus={selectedTeamRuntimeStatus}
      selectedTeamHasConfiguredMembers={selectedTeamHasConfiguredMembers}
      teamMemberSummaryByTeamId={teamMemberSummaryByTeamId}
      memberLiveStates={selectedTeamMemberLiveStates}
      channelItems={channelItems}
      selectedChannelId={routeChannelId}
      focusedAgentMemberId={focusedAgentMemberId}
      activeWorkspaceLens={routeWorkspaceLens ?? undefined}
      tab={tab}
      onSelectTeam={navigateToTeamDetail}
      onBackToSelector={navigateToTeamSelector}
      onSelectChannel={onSelectChannel}
      onCreateChannel={onCreateChannel}
      onDeleteChannel={onDeleteChannel}
      creatingChannel={creatingChannel}
      deletingChannelId={deletingChannelId}
      onSelectKanban={onSelectKanbanSubject}
      onSelectSearch={() => onSelectWorkspaceLens("search")}
      onSelectAgentTab={onSelectAgentWorkspace}
      onOpenTeamMemberForge={onOpenTeamMemberForge}
      onOpenTeamMemberCopyExisting={onOpenTeamMemberCopyExisting}
      teamMemberForgeLabel={teamMemberForgeLabel}
      teamMemberCopyExistingLabel={teamMemberCopyExistingLabel}
      onStartTeamRuntime={onStartTeamRuntime}
      onStopTeamRuntime={onStopTeamRuntime}
      onOpenMachines={onOpenMachines}
      currentMachineId={currentMachineId}
      onOpenCurrentMachine={onOpenCurrentMachine}
    />
  );
});
