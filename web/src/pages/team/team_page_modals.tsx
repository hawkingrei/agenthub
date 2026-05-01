import React from "react";
import {
  TeamCreateDialog,
  TeamEditMemberDialog,
  TeamForgeAgentDialog,
  type TeamModalChrome,
} from "./team_management_modals";
import type { TeamMemberProfileDraft } from "./create_helpers";
import type { TeamMemberRoleOption, TeamMemberRoleProfile } from "./forge_helpers";
import type { CreateAgentModalProps } from "../../components/create_agent_modal";

type TeamPageModalsProps = {
  showCreateTeamModal: boolean;
  showForgeAgentForm: boolean;
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
  teamMemberRoleOptions: TeamMemberRoleOption[];
  selectedTeamHasCoordinator: boolean;
  handleTeamMemberRoleChange: (value: string) => void;
  patchTeamMemberDraft: (patch: Partial<TeamMemberProfileDraft>) => void;
  forgeModalProps: Omit<CreateAgentModalProps, "children" | "onClose">;
  closeTeamMemberForgeModal: () => void;
  selectedAgentLabel: string;
  teamMemberEditDraft: TeamMemberProfileDraft | null;
  patchTeamMemberEditDraft: (patch: Partial<TeamMemberProfileDraft>) => void;
  closeTeamMemberEditModal: () => void;
  onSaveTeamMemberProfile: () => void;
  createChrome: TeamModalChrome;
  forgeChrome: TeamModalChrome;
  editChrome: TeamModalChrome;
};

export const TeamPageModals = React.memo(function TeamPageModals({
  showCreateTeamModal,
  showForgeAgentForm,
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
  teamMemberRoleOptions,
  selectedTeamHasCoordinator,
  handleTeamMemberRoleChange,
  patchTeamMemberDraft,
  forgeModalProps,
  closeTeamMemberForgeModal,
  selectedAgentLabel,
  teamMemberEditDraft,
  patchTeamMemberEditDraft,
  closeTeamMemberEditModal,
  onSaveTeamMemberProfile,
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
        roleOptions={teamMemberRoleOptions}
        selectedTeamHasCoordinator={selectedTeamHasCoordinator}
        onRoleChange={handleTeamMemberRoleChange}
        onPatchDraft={patchTeamMemberDraft}
        chrome={forgeChrome}
        modalProps={{
          ...forgeModalProps,
          onClose: closeTeamMemberForgeModal,
        }}
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
      />
    </>
  );
});
