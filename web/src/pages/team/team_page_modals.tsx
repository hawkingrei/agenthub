import React from "react";
import {
  TeamCreateDialog,
  TeamCopyExistingAgentDialog,
  TeamEditMemberDialog,
  TeamForgeAgentDialog,
  type TeamModalChrome,
} from "./team_management_modals";
import type { TeamMemberProfileDraft } from "./create_helpers";
import type { TeamMemberRoleProfile } from "./forge_helpers";
import type { AgentRecord } from "../../api";
import type { CreateAgentModalProps } from "../../components/create_agent_modal";

export type TeamPageModalsProps = {
  showCreateTeamModal: boolean;
  showForgeAgentForm: boolean;
  showCopyExistingAgentModal: boolean;
  showTeamMemberEditModal: boolean;
  busy: string | null;
  newTeamName: string;
  newTeamDescription: string;
  onTeamNameChange: (value: string) => void;
  onTeamDescriptionChange: (value: string) => void;
  onCreateTeam: () => void;
  closeCreateTeamModal: () => void;
  teamMemberDraft: TeamMemberProfileDraft | null;
  teamMemberRoleProfile: TeamMemberRoleProfile | null;
  selectedTeamHasCoordinator: boolean;
  copyExistingCandidates: AgentRecord[];
  patchTeamMemberDraft: (patch: Partial<TeamMemberProfileDraft>) => void;
  forgeModalProps: Omit<CreateAgentModalProps, "children" | "onClose">;
  closeTeamMemberForgeModal: () => void;
  closeCopyExistingAgentModal: () => void;
  onCopyExistingAgent: (agentId: string) => void;
  onMoveExistingAgent: (agentId: string) => void;
  selectedAgentLabel: string;
  teamMemberEditDraft: TeamMemberProfileDraft | null;
  patchTeamMemberEditDraft: (patch: Partial<TeamMemberProfileDraft>) => void;
  closeTeamMemberEditModal: () => void;
  onSaveTeamMemberProfile: () => void;
  selectedAgentSupportsRuntimeProfile: boolean;
  selectedAgentIsCodex: boolean;
  createChrome: TeamModalChrome;
  forgeChrome: TeamModalChrome;
  editChrome: TeamModalChrome;
};

export const TeamPageModals = React.memo(function TeamPageModals({
  showCreateTeamModal,
  showForgeAgentForm,
  showCopyExistingAgentModal,
  showTeamMemberEditModal,
  busy,
  newTeamName,
  newTeamDescription,
  onTeamNameChange,
  onTeamDescriptionChange,
  onCreateTeam,
  closeCreateTeamModal,
  teamMemberDraft,
  teamMemberRoleProfile,
  selectedTeamHasCoordinator,
  copyExistingCandidates,
  patchTeamMemberDraft,
  forgeModalProps,
  closeTeamMemberForgeModal,
  closeCopyExistingAgentModal,
  onCopyExistingAgent,
  onMoveExistingAgent,
  selectedAgentLabel,
  teamMemberEditDraft,
  patchTeamMemberEditDraft,
  closeTeamMemberEditModal,
  onSaveTeamMemberProfile,
  selectedAgentSupportsRuntimeProfile,
  selectedAgentIsCodex,
  createChrome,
  forgeChrome,
  editChrome,
}: TeamPageModalsProps) {
  return (
    <>
      <TeamCreateDialog
        open={showCreateTeamModal}
        busy={busy}
        teamName={newTeamName}
        teamDescription={newTeamDescription}
        onTeamNameChange={onTeamNameChange}
        onTeamDescriptionChange={onTeamDescriptionChange}
        onCreateTeam={onCreateTeam}
        onClose={closeCreateTeamModal}
        chrome={createChrome}
      />
      <TeamForgeAgentDialog
        open={showForgeAgentForm}
        draft={teamMemberDraft}
        roleProfile={teamMemberRoleProfile}
        selectedTeamHasCoordinator={selectedTeamHasCoordinator}
        onPatchDraft={patchTeamMemberDraft}
        chrome={forgeChrome}
        modalProps={{
          ...forgeModalProps,
          onClose: closeTeamMemberForgeModal,
        }}
      />
      <TeamCopyExistingAgentDialog
        open={showCopyExistingAgentModal}
        busy={busy === "copy-team-agent" || busy === "move-team-agent"}
        selectedTeamHasCoordinator={selectedTeamHasCoordinator}
        candidateAgents={copyExistingCandidates}
        onCopy={onCopyExistingAgent}
        onMove={onMoveExistingAgent}
        onClose={closeCopyExistingAgentModal}
        chrome={forgeChrome}
      />
      <TeamEditMemberDialog
        open={showTeamMemberEditModal}
        busy={busy}
        selectedAgentLabel={selectedAgentLabel}
        draft={teamMemberEditDraft}
        onPatchDraft={patchTeamMemberEditDraft}
        onClose={closeTeamMemberEditModal}
        onSave={onSaveTeamMemberProfile}
        chrome={editChrome}
        supportsRuntimeProfile={selectedAgentSupportsRuntimeProfile}
        isCodexProvider={selectedAgentIsCodex}
      />
    </>
  );
});
