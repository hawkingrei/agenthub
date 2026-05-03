import React from "react";
import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

vi.mock("../../components/create_agent_modal", () => ({
  CreateAgentModal: ({
    title,
    children,
  }: {
    title?: string;
    children?: React.ReactNode;
  }) => (
    <div data-testid="mock-create-agent-modal">
      <h2>{title}</h2>
      {children}
    </div>
  ),
}));

import {
  TeamCreateDialog,
  TeamEditMemberDialog,
  TeamForgeAgentDialog,
} from "./team_management_modals";

const chrome = {
  panelClassName: "panel",
  accentButtonClassName: "accent",
  mutedButtonClassName: "muted",
  badgeClassName: "badge",
  modalHeaderClassName: "modal-head",
  setupChecklistClassName: "checklist",
  infoStripGridClassName: "info-grid",
  infoStripItemClassName: "info-item",
  infoStripLabelClassName: "info-label",
  infoStripValueClassName: "info-value",
};

describe("Team management modals", () => {
  it("renders the create-team dialog with disabled submit until a name is provided", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamCreateDialog
          open
          busy={null}
          teamName=""
          teamDescription=""
          onTeamNameChange={vi.fn()}
          onTeamDescriptionChange={vi.fn()}
          onCreateTeam={vi.fn()}
          onClose={vi.fn()}
          chrome={chrome}
        />
      </MantineProvider>
    );

    expect(html).toContain("Start with the mission, not the agents.");
    expect(html).toContain("Team name is required before the team can be created.");
    expect(html).toContain("Create Team");
    expect(html).toContain("disabled");
  });

  it("renders the create-team dialog with first-agent coordinator guidance once named", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamCreateDialog
          open
          busy={null}
          teamName="growth-hive"
          teamDescription=""
          onTeamNameChange={vi.fn()}
          onTeamDescriptionChange={vi.fn()}
          onCreateTeam={vi.fn()}
          onClose={vi.fn()}
          chrome={chrome}
        />
      </MantineProvider>
    );

    expect(html).toContain("with the first added agent becoming the coordinator by default");
    expect(html).toContain(
      "After the team is created, add the first agent as coordinator. More agents can be added after that."
    );
  });

  it("renders the edit-member dialog with current profile values", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamEditMemberDialog
          open
          busy={null}
          selectedAgentLabel="worker-1"
          draft={{
            member_id: "worker-1",
            role: "worker",
            description: "Fix optimizer regressions",
            model: "gpt-5.4",
            prompt: "Stay focused on optimizer issues.",
            skills: [],
            custom_skills: "",
            agent_loop_enabled: true,
            agent_loop_idle_seconds: "900",
            agent_loop_prompt: "Resume by checking inbox.",
          }}
          onPatchDraft={vi.fn()}
          onClose={vi.fn()}
          onSave={vi.fn()}
          chrome={chrome}
        />
      </MantineProvider>
    );

    expect(html).toContain("Edit worker-1");
    expect(html).toContain("gpt-5.4");
    expect(html).toContain("Description");
    expect(html).toContain("What should this agent help with?");
    expect(html).toContain("Save Profile");
    expect(html).toContain("Role-bound Team skills come from the system-managed skill path");
  });

  it("renders the forge-agent dialog with role guidance and system skills", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamForgeAgentDialog
          open
          draft={{
            member_id: "coordinator-1",
            role: "coordinator",
            description: "Own planning",
            model: "",
            prompt: "Plan and coordinate.",
            skills: [],
            custom_skills: "",
            agent_loop_enabled: false,
            agent_loop_idle_seconds: "",
            agent_loop_prompt: "",
          }}
          roleProfile={{
            profileLabel: "Coordinator Profile",
            intro: "Own coordination.",
            focus: "Planning",
            skillsHint: "Defaults are managed for you.",
            promptHint: "Describe what this agent should own.",
          }}
          roleOptions={[
            {
              value: "coordinator",
              label: "Coordinator",
              description: "Own planning and review.",
              disabled: false,
            },
            {
              value: "worker",
              label: "Worker",
              description: "Execute scoped tasks.",
              disabled: false,
            },
          ]}
          selectedTeamHasCoordinator={false}
          onRoleChange={vi.fn()}
          onPatchDraft={vi.fn()}
          chrome={{
            ...chrome,
            setupChecklistClassName: "checklist",
            infoStripGridClassName: "info-grid",
          }}
          modalProps={{
            title: "Add Agent",
            confirmLabel: "Create Agent",
            agentPresetLabel: "Runtime",
            agentPresetSummaryLabel: "Model",
            showCommandSummary: false,
            teamStyled: true,
            agentName: "coordinator-1",
            setAgentName: vi.fn(),
            agentWorkdir: "/repo",
            setAgentWorkdir: vi.fn(),
            agentPresetId: "codex",
            setAgentPresetId: vi.fn(),
            worktreeMode: "use_existing",
            setWorktreeMode: vi.fn(),
            worktreeRepo: "",
            setWorktreeRepo: vi.fn(),
            worktreeRef: "",
            setWorktreeRef: vi.fn(),
            codeMode: true,
            setCodeMode: vi.fn(),
            worktreeError: null,
            showWorktreeAdvancedOptions: false,
            createBusy: false,
            workdirPlaceholder: "~/.agenthub/worktrees",
            withinPortal: true,
            onCreateAgent: vi.fn(),
            onClose: vi.fn(),
          }}
        />
      </MantineProvider>
    );

    expect(html).toContain("Coordinator Profile");
    expect(html).toContain("Managed automatically");
    expect(html).toContain("What should this agent help with?");
    expect(html).toContain("only asks for a description and workspace settings");
    expect(html).toContain("First agent = coordinator");
    expect(html).toContain("The first agent you add becomes the coordinator. Worker unlocks after that.");
    expect(html).toContain("The first agent added to a Team becomes the coordinator.");
    expect(html).not.toContain("Prompt Scope");
    expect(html).not.toContain("Launch command");
  });

  it("renders the forge-agent dialog with worker guidance once a coordinator exists", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamForgeAgentDialog
          open
          draft={{
            member_id: "worker-2",
            role: "worker",
            description: "Handle implementation follow-up",
            model: "",
            prompt: "Execute scoped tasks.",
            skills: [],
            custom_skills: "",
            agent_loop_enabled: false,
            agent_loop_idle_seconds: "",
            agent_loop_prompt: "",
          }}
          roleProfile={{
            profileLabel: "Worker Profile",
            intro: "Execute scoped tasks.",
            focus: "Implementation",
            skillsHint: "Defaults are managed for you.",
            promptHint: "Describe what this worker should deliver.",
          }}
          roleOptions={[
            {
              value: "coordinator",
              label: "Coordinator",
              description: "Own planning and review.",
              disabled: true,
            },
            {
              value: "worker",
              label: "Worker",
              description: "Execute scoped tasks.",
              disabled: false,
            },
          ]}
          selectedTeamHasCoordinator
          onRoleChange={vi.fn()}
          onPatchDraft={vi.fn()}
          chrome={{
            ...chrome,
            setupChecklistClassName: "checklist",
            infoStripGridClassName: "info-grid",
          }}
          modalProps={{
            title: "Add Worker Agent",
            confirmLabel: "Create Worker Agent",
            agentPresetLabel: "Runtime",
            agentPresetSummaryLabel: "Model",
            showCommandSummary: false,
            teamStyled: true,
            agentName: "worker-2",
            setAgentName: vi.fn(),
            agentWorkdir: "/repo",
            setAgentWorkdir: vi.fn(),
            agentPresetId: "codex",
            setAgentPresetId: vi.fn(),
            worktreeMode: "use_existing",
            setWorktreeMode: vi.fn(),
            worktreeRepo: "",
            setWorktreeRepo: vi.fn(),
            worktreeRef: "",
            setWorktreeRef: vi.fn(),
            codeMode: true,
            setCodeMode: vi.fn(),
            worktreeError: null,
            showWorktreeAdvancedOptions: false,
            createBusy: false,
            workdirPlaceholder: "~/.agenthub/worktrees",
            withinPortal: true,
            onCreateAgent: vi.fn(),
            onClose: vi.fn(),
          }}
        />
      </MantineProvider>
    );

    expect(html).toContain("Worker Profile");
    expect(html).toContain("Execution role");
    expect(html).toContain("This team already has a coordinator. New agents join as workers.");
    expect(html).not.toContain("Coordinator default");
    expect(html).not.toContain("First agent = coordinator");
  });
});
