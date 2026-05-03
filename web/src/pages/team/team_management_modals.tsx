import React from "react";
import { Alert, Switch, TextInput, Textarea } from "@mantine/core";
import { DEFAULT_TEAM_COORDINATOR_SKILLS, DEFAULT_TEAM_WORKER_SKILLS } from "./member_helpers";
import type { TeamMemberProfileDraft } from "./create_helpers";
import type { TeamMemberRoleProfile } from "./forge_helpers";
import type { AgentRecord } from "../../api";
import {
  CreateAgentModal,
  type CreateAgentModalProps,
} from "../../components/create_agent_modal";
import {
  TEAM_CREATE_ACTIONS_BAR_CLASS,
  TEAM_CREATE_MODAL_BACKDROP_CLASS,
  TEAM_CREATE_MODAL_CARD_CLASS,
  TEAM_CREATE_PANEL_CARD_CLASS,
  TEAM_CREATE_SKILL_TAG_SELECTED_CLASS,
} from "../../ui/tailwind_classes";
import { ActionButton, BetaBadge, SurfaceCard } from "../../ui/primitives";

type TeamCreateNoteTone = "info" | "warning";

export type TeamModalChrome = {
  panelClassName: string;
  accentButtonClassName: string;
  mutedButtonClassName: string;
  badgeClassName: string;
  modalHeaderClassName: string;
  setupChecklistClassName: string;
  infoStripGridClassName: string;
  infoStripItemClassName: string;
  infoStripLabelClassName: string;
  infoStripValueClassName: string;
};

const TEAM_CREATE_NOTE_ALERT_CONFIG: Record<
  TeamCreateNoteTone,
  { color: "blue" | "yellow"; title: string; iconClassName: string }
> = {
  info: {
    color: "blue",
    title: "Team note",
    iconClassName: "bi bi-info-circle",
  },
  warning: {
    color: "yellow",
    title: "Action required",
    iconClassName: "bi bi-exclamation-triangle",
  },
};

const TeamCreateNote = React.memo(function TeamCreateNote({
  tone,
  children,
}: {
  tone: TeamCreateNoteTone;
  children: React.ReactNode;
}) {
  const config = TEAM_CREATE_NOTE_ALERT_CONFIG[tone];
  return (
    <Alert
      color={config.color}
      variant="light"
      radius="md"
      mt="md"
      title={config.title}
      icon={<i className={config.iconClassName} aria-hidden="true" />}
    >
      <div className="text-sm">{children}</div>
    </Alert>
  );
});

export const TeamCreateDialog = React.memo(function TeamCreateDialog({
  open,
  busy,
  teamName,
  teamDescription,
  onTeamNameChange,
  onTeamDescriptionChange,
  onCreateTeam,
  onClose,
  chrome,
}: {
  open: boolean;
  busy: string | null;
  teamName: string;
  teamDescription: string;
  onTeamNameChange: (value: string) => void;
  onTeamDescriptionChange: (value: string) => void;
  onCreateTeam: () => void;
  onClose: () => void;
  chrome: TeamModalChrome;
}) {
  if (!open) {
    return null;
  }

  const createBusy = busy === "create-team";

  return (
    <div
      className={TEAM_CREATE_MODAL_BACKDROP_CLASS}
      role="presentation"
      onClick={(event) => {
        if (event.target === event.currentTarget && !createBusy) {
          onClose();
        }
      }}
    >
      <div
        className={`${TEAM_CREATE_MODAL_CARD_CLASS} ${chrome.panelClassName}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="team-create-title"
      >
        <div className={chrome.modalHeaderClassName}>
          <div className="min-w-0 flex-1">
            <span className={chrome.badgeClassName}>Create Team</span>
            <h3
              id="team-create-title"
              className="mt-2 text-[18px] font-semibold tracking-tight text-black"
            >
              Start with the mission, not the agents.
            </h3>
            <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/70">
              Team creation only stores the workspace identity and goal. Add agents afterward,
              with the first added agent becoming the coordinator by default.
            </p>
          </div>
        </div>
        <div className="modal-body mt-4 space-y-4">
          <SurfaceCard className={TEAM_CREATE_PANEL_CARD_CLASS}>
            <TextInput
              label="Team name"
              radius="md"
              placeholder="growth-hive"
              value={teamName}
              onChange={(event) => onTeamNameChange(event.target.value)}
            />
            <Textarea
              className="mt-3"
              label="Team goal"
              radius="md"
              minRows={5}
              autosize
              placeholder="Describe the mission, constraints, and what this team should own."
              value={teamDescription}
              onChange={(event) => onTeamDescriptionChange(event.target.value)}
            />
          </SurfaceCard>

          <TeamCreateNote tone={teamName.trim() ? "info" : "warning"}>
            {teamName.trim()
              ? "After the team is created, add the first agent as coordinator. More agents can be added after that."
              : "Team name is required before the team can be created."}
          </TeamCreateNote>
        </div>

        <div className={TEAM_CREATE_ACTIONS_BAR_CLASS}>
          <ActionButton
            className={chrome.mutedButtonClassName}
            onClick={onClose}
            disabled={createBusy}
            size="md"
            tone="secondary"
            type="button"
          >
            Cancel
          </ActionButton>
          <ActionButton
            className={chrome.accentButtonClassName}
            onClick={onCreateTeam}
            disabled={createBusy || !teamName.trim()}
            size="md"
            tone="primary"
            type="button"
          >
            {createBusy ? "Creating..." : "Create Team"}
          </ActionButton>
        </div>
      </div>
    </div>
  );
});

export const TeamEditMemberDialog = React.memo(function TeamEditMemberDialog({
  open,
  busy,
  selectedAgentLabel,
  draft,
  onPatchDraft,
  onClose,
  onSave,
  chrome,
}: {
  open: boolean;
  busy: string | null;
  selectedAgentLabel: string;
  draft: TeamMemberProfileDraft | null;
  onPatchDraft: (patch: Partial<TeamMemberProfileDraft>) => void;
  onClose: () => void;
  onSave: () => void;
  chrome: TeamModalChrome;
}) {
  if (!open || !draft) {
    return null;
  }

  const saveBusy = busy === "save-team-member-profile";

  return (
    <div
      className={TEAM_CREATE_MODAL_BACKDROP_CLASS}
      role="presentation"
      onClick={(event) => {
        if (event.target === event.currentTarget && !saveBusy) {
          onClose();
        }
      }}
    >
      <div
        className={`${TEAM_CREATE_MODAL_CARD_CLASS} ${chrome.panelClassName}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="team-edit-member-title"
      >
        <div className={chrome.modalHeaderClassName}>
          <div className="min-w-0 flex-1">
            <span className={chrome.badgeClassName}>Agent Profile</span>
            <h3
              id="team-edit-member-title"
              className="mt-2 text-[18px] font-semibold tracking-tight text-black"
            >
              Edit {selectedAgentLabel}
            </h3>
            <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/70">
              Update the Team-owned agent identity, prompt, and skill profile without leaving
              the current workspace.
            </p>
          </div>
        </div>
        <div className="modal-body mt-4 space-y-4">
          <SurfaceCard className={`${TEAM_CREATE_PANEL_CARD_CLASS} border border-ui-border bg-ui-surface/90`}>
            <div className="grid gap-px overflow-hidden rounded-[14px] border border-ui-border bg-ui-border sm:grid-cols-3">
              {[
                { label: "Member", value: draft.member_id },
                { label: "Role", value: draft.role },
                { label: "Model", value: draft.model.trim() || "-" },
              ].map((item) => (
                <div key={item.label} className={chrome.infoStripItemClassName}>
                  <p className={chrome.infoStripLabelClassName}>{item.label}</p>
                  <p className={chrome.infoStripValueClassName}>{item.value}</p>
                </div>
              ))}
            </div>

            <TextInput
              className="mt-4"
              radius="md"
              label="Description"
              placeholder="What should this agent help with?"
              value={draft.description}
              onChange={(event) =>
                onPatchDraft({ description: event.currentTarget.value })
              }
            />
            <TextInput
              className="mt-3"
              radius="md"
              label="Model"
              placeholder="Optional model override"
              value={draft.model}
              onChange={(event) => onPatchDraft({ model: event.currentTarget.value })}
            />
            <SurfaceCard className="mt-4 rounded-[14px] border border-ui-border bg-ui-surface-soft/70 p-4 shadow-none">
              <div className="flex flex-col gap-3">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="min-w-0 flex-1">
                    <p className={chrome.infoStripLabelClassName}>Agent loop</p>
                    <p className="mt-1 text-[12px] leading-5 text-ui-text-secondary">
                      When enabled, AgentHub will inject the configured ACP prompt after this
                      agent stays silent for the selected idle window.
                    </p>
                  </div>
                  <Switch
                    checked={draft.agent_loop_enabled}
                    onChange={(event) =>
                      onPatchDraft({
                        agent_loop_enabled: event.currentTarget.checked,
                      })
                    }
                    label={draft.agent_loop_enabled ? "Enabled" : "Disabled"}
                  />
                </div>
                <TextInput
                  radius="md"
                  label="Idle timeout (seconds)"
                  placeholder="900"
                  value={draft.agent_loop_idle_seconds}
                  disabled={!draft.agent_loop_enabled}
                  onChange={(event) =>
                    onPatchDraft({
                      agent_loop_idle_seconds: event.currentTarget.value,
                    })
                  }
                />
                <Textarea
                  radius="md"
                  minRows={3}
                  autosize
                  label="Loop prompt"
                  placeholder="You have been idle. Resume by checking your inbox, summarizing current state, and taking the next scoped action."
                  value={draft.agent_loop_prompt}
                  disabled={!draft.agent_loop_enabled}
                  onChange={(event) =>
                    onPatchDraft({
                      agent_loop_prompt: event.currentTarget.value,
                    })
                  }
                />
              </div>
            </SurfaceCard>
            <SurfaceCard className="mt-4 rounded-lg border border-notion-border bg-notion-sidebar/30 p-3 shadow-none">
              <p className={chrome.infoStripLabelClassName}>System Skills</p>
              <p className="mt-1 text-[12px] leading-5 text-ui-text-secondary">
                Role-bound Team skills come from the system-managed skill path and are shown here
                as the effective runtime contract.
              </p>
              <div className="mt-3 flex flex-wrap gap-2">
                {(draft.role === "coordinator"
                  ? DEFAULT_TEAM_COORDINATOR_SKILLS
                  : DEFAULT_TEAM_WORKER_SKILLS
                ).map((skill) => (
                  <span
                    key={`${draft.role}-edit-system-skill-${skill}`}
                    className={TEAM_CREATE_SKILL_TAG_SELECTED_CLASS}
                  >
                    {skill}
                  </span>
                ))}
              </div>
            </SurfaceCard>
            <Textarea
              className="mt-3"
              radius="md"
              label="Prompt"
              minRows={6}
              autosize
              value={draft.prompt}
              onChange={(event) => onPatchDraft({ prompt: event.currentTarget.value })}
              styles={{
                input: {
                  fontFamily: "monospace",
                  fontSize: "12px",
                  lineHeight: "1.5",
                },
              }}
            />
          </SurfaceCard>
        </div>

        <div className={TEAM_CREATE_ACTIONS_BAR_CLASS}>
          <ActionButton
            className={chrome.mutedButtonClassName}
            onClick={onClose}
            disabled={saveBusy}
            size="md"
            tone="secondary"
            type="button"
          >
            Cancel
          </ActionButton>
          <ActionButton
            className={chrome.accentButtonClassName}
            onClick={onSave}
            disabled={saveBusy}
            size="md"
            tone="primary"
            type="button"
          >
            {saveBusy ? "Saving..." : "Save Profile"}
          </ActionButton>
        </div>
      </div>
    </div>
  );
});

export const TeamCopyExistingAgentDialog = React.memo(function TeamCopyExistingAgentDialog({
  open,
  busy,
  selectedTeamHasCoordinator,
  candidateAgents,
  onCopy,
  onClose,
  chrome,
}: {
  open: boolean;
  busy: boolean;
  selectedTeamHasCoordinator: boolean;
  candidateAgents: AgentRecord[];
  onCopy: (agentId: string) => void;
  onClose: () => void;
  chrome: TeamModalChrome;
}) {
  const [filter, setFilter] = React.useState("");
  const [selectedAgentId, setSelectedAgentId] = React.useState("");

  React.useEffect(() => {
    if (!open) {
      setFilter("");
      setSelectedAgentId("");
    }
  }, [open]);

  const filteredAgents = React.useMemo(() => {
    const normalizedFilter = filter.trim().toLowerCase();
    if (!normalizedFilter) {
      return candidateAgents;
    }
    return candidateAgents.filter((agent) => {
      const haystacks = [
        agent.name,
        agent.id,
        agent.workdir,
        agent.target_node_id ?? "",
      ].map((value) => value.toLowerCase());
      return haystacks.some((value) => value.includes(normalizedFilter));
    });
  }, [candidateAgents, filter]);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    if (selectedAgentId && filteredAgents.some((agent) => agent.id === selectedAgentId)) {
      return;
    }
    setSelectedAgentId(filteredAgents[0]?.id ?? "");
  }, [filteredAgents, open, selectedAgentId]);

  if (!open) {
    return null;
  }

  const copyRoleLabel = selectedTeamHasCoordinator ? "worker" : "coordinator";

  return (
    <div
      className={TEAM_CREATE_MODAL_BACKDROP_CLASS}
      role="presentation"
      onClick={(event) => {
        if (event.target === event.currentTarget && !busy) {
          onClose();
        }
      }}
    >
      <div
        className={`${TEAM_CREATE_MODAL_CARD_CLASS} ${chrome.panelClassName}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="team-copy-existing-agent-title"
      >
        <div className={chrome.modalHeaderClassName}>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <span className={chrome.badgeClassName}>Add Existing Agent</span>
              <BetaBadge />
            </div>
            <h3
              id="team-copy-existing-agent-title"
              className="mt-2 text-[18px] font-semibold tracking-tight text-black"
            >
              Copy an existing agent into this team.
            </h3>
            <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/70">
              AgentHub will create a new Team-owned {copyRoleLabel} agent from the selected source
              configuration. The original agent remains unchanged.
            </p>
          </div>
        </div>

        <div className="modal-body mt-4 space-y-4">
          <SurfaceCard className={TEAM_CREATE_PANEL_CARD_CLASS}>
            <TextInput
              label="Search existing agents"
              radius="md"
              placeholder="Filter by name, id, workspace, or node"
              value={filter}
              onChange={(event) => setFilter(event.currentTarget.value)}
            />

            <div className="mt-4 space-y-3">
              {filteredAgents.length === 0 ? (
                <Alert radius="md" color="blue" variant="light" title="No matching agents">
                  No existing agents match this filter. Create a new Team-owned agent instead.
                </Alert>
              ) : (
                filteredAgents.map((agent) => {
                  const selected = agent.id === selectedAgentId;
                  return (
                    <button
                      key={agent.id}
                      type="button"
                      className={`w-full rounded-[14px] border px-3.5 py-3 text-left transition ${
                        selected
                          ? "border-ui-border-emphasis bg-ui-surface-soft shadow-sm"
                          : "border-ui-border bg-white hover:border-ui-border-emphasis hover:bg-ui-surface-soft"
                      }`}
                      onClick={() => setSelectedAgentId(agent.id)}
                    >
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <div className="min-w-0">
                          <p className="truncate text-[13px] font-semibold text-ui-text-primary">
                            {agent.name}
                          </p>
                          <p className="mt-1 truncate text-[11px] text-ui-text-muted">
                            {agent.id}
                          </p>
                        </div>
                        <span className={chrome.badgeClassName}>
                          {selected ? "Selected" : "Existing Agent"}
                        </span>
                      </div>
                      <div className="mt-3 grid gap-px overflow-hidden rounded-[12px] border border-ui-border bg-ui-border sm:grid-cols-3">
                        <div className={chrome.infoStripItemClassName}>
                          <p className={chrome.infoStripLabelClassName}>Workspace</p>
                          <p className={chrome.infoStripValueClassName}>{agent.workdir || "-"}</p>
                        </div>
                        <div className={chrome.infoStripItemClassName}>
                          <p className={chrome.infoStripLabelClassName}>Mode</p>
                          <p className={chrome.infoStripValueClassName}>{agent.worktree_mode}</p>
                        </div>
                        <div className={chrome.infoStripItemClassName}>
                          <p className={chrome.infoStripLabelClassName}>Node</p>
                          <p className={chrome.infoStripValueClassName}>
                            {agent.target_node_id?.trim() || "main"}
                          </p>
                        </div>
                      </div>
                    </button>
                  );
                })
              )}
            </div>
          </SurfaceCard>

          <Alert radius="md" color="blue" variant="light" title="Copy semantics">
            Copy keeps the source agent unchanged. AgentHub creates a new Team-owned{" "}
            {copyRoleLabel} agent with copied runtime settings and fresh Team membership.
          </Alert>
        </div>

        <div className={TEAM_CREATE_ACTIONS_BAR_CLASS}>
          <ActionButton
            className={chrome.mutedButtonClassName}
            onClick={onClose}
            disabled={busy}
            size="md"
            tone="secondary"
            type="button"
          >
            Cancel
          </ActionButton>
          <ActionButton
            className={chrome.accentButtonClassName}
            onClick={() => onCopy(selectedAgentId)}
            disabled={busy || !selectedAgentId}
            size="md"
            tone="primary"
            type="button"
          >
            {busy ? "Copying..." : "Copy into Team"}
          </ActionButton>
        </div>
      </div>
    </div>
  );
});

export const TeamForgeAgentDialog = React.memo(function TeamForgeAgentDialog({
  open,
  draft,
  roleProfile,
  selectedTeamHasCoordinator,
  onPatchDraft,
  chrome,
  modalProps,
}: {
  open: boolean;
  draft: TeamMemberProfileDraft | null;
  roleProfile: TeamMemberRoleProfile | null;
  selectedTeamHasCoordinator: boolean;
  onPatchDraft: (patch: Partial<TeamMemberProfileDraft>) => void;
  chrome: TeamModalChrome;
  modalProps: Omit<CreateAgentModalProps, "children">;
}) {
  if (!open || !draft) {
    return null;
  }

  return (
    <CreateAgentModal {...modalProps}>
      <SurfaceCard className={`${TEAM_CREATE_PANEL_CARD_CLASS} border border-ui-border bg-ui-surface/90`}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <span className={chrome.badgeClassName}>
            {roleProfile?.profileLabel ?? "Agent Profile"}
          </span>
          <span className="text-xs font-medium uppercase tracking-[0.14em] text-ui-text-muted">
            member_id follows agent id
          </span>
        </div>
        <p className="mt-2 text-[13px] leading-5 text-ui-text-secondary">
          {roleProfile?.intro ??
            "Add the agent identity before attaching it to the team."}
        </p>
        <SurfaceCard className="mt-4 rounded-[14px] border border-ui-border bg-ui-surface-soft px-3.5 py-3 shadow-none">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="min-w-0">
              <p className={chrome.infoStripLabelClassName}>Assigned Role</p>
              <p className="mt-1 text-[12px] leading-5 text-ui-text-secondary">
                {selectedTeamHasCoordinator
                  ? "This team already has a coordinator, so new agents join as workers by default."
                  : "This team does not have a coordinator yet, so the first added agent becomes the coordinator automatically."}
              </p>
            </div>
            <span className={chrome.badgeClassName}>
              {draft.role === "coordinator" ? "Coordinator" : "Worker"}
            </span>
          </div>
          <p className="mt-3 text-[11px] leading-5 text-ui-text-muted">
            The default Add Agent flow keeps role assignment fixed so the modal only asks for
            description and workspace settings.
          </p>
        </SurfaceCard>
        <div className={`${chrome.setupChecklistClassName} mt-4`}>
          <div className={chrome.infoStripGridClassName}>
            <div className={chrome.infoStripItemClassName}>
              <p className={chrome.infoStripLabelClassName}>Focus</p>
              <p className={chrome.infoStripValueClassName}>
                {roleProfile?.focus ?? "Set the role before editing the profile details."}
              </p>
            </div>
            <div className={chrome.infoStripItemClassName}>
              <p className={chrome.infoStripLabelClassName}>Defaults</p>
              <p className={chrome.infoStripValueClassName}>
                {roleProfile?.skillsHint ??
                  "Role skills and system instructions are managed automatically."}
              </p>
            </div>
            <div className={chrome.infoStripItemClassName}>
              <p className={chrome.infoStripLabelClassName}>Description</p>
              <p className={chrome.infoStripValueClassName}>
                {roleProfile?.promptHint ??
                  "Describe the scope, responsibilities, or working style in plain language."}
              </p>
            </div>
          </div>
        </div>

        <TextInput
          className="mt-4"
          radius="md"
          label="Description"
          placeholder="What should this agent help with?"
          value={draft.description}
          onChange={(event) =>
            onPatchDraft({ description: event.currentTarget.value })
          }
        />
        <Alert className="mt-4" radius="md" color="blue" variant="light" title="Managed automatically">
          AgentHub injects the role-specific system prompt and Team skills automatically for the
          selected role. The default Add Agent flow only asks for a description and workspace
          settings.
        </Alert>
        {!selectedTeamHasCoordinator ? (
          <Alert
            className="mt-3"
            radius="md"
            color="blue"
            variant="light"
            title="Coordinator default"
          >
            The first agent added to a Team becomes the coordinator. Add workers after this first
            agent exists.
          </Alert>
        ) : null}
      </SurfaceCard>
    </CreateAgentModal>
  );
});
