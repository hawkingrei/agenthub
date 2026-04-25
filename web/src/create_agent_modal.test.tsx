import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
import {
  CreateAgentModal,
  type CreateAgentModalProps,
  resolveCreateAgentPresetId,
  resolveCreateAgentWorktreeMode,
  shouldAutoExpandCreateAgentAdvancedOptions,
} from "./components/create_agent_modal";

const baseProps: CreateAgentModalProps = {
  agentName: "",
  setAgentName: () => {},
  agentWorkdir: "",
  setAgentWorkdir: () => {},
  agentPresetId: "codex" as const,
  setAgentPresetId: () => {},
  worktreeMode: "use_existing" as const,
  setWorktreeMode: () => {},
  worktreeRepo: "",
  setWorktreeRepo: () => {},
  worktreeRef: "",
  setWorktreeRef: () => {},
  codeMode: false,
  setCodeMode: () => {},
  worktreeError: null as string | null,
  createBusy: false,
  workdirPlaceholder: "Workdir",
  withinPortal: false,
  onCreateAgent: () => {},
  onClose: () => {},
};

const renderModal = (overrides?: Partial<CreateAgentModalProps>) =>
  renderToStaticMarkup(
    <MantineProvider>
      <CreateAgentModal {...baseProps} {...overrides} />
    </MantineProvider>
  );

describe("CreateAgentModal", () => {
  it("renders repo and ref inputs for create_worktree", () => {
    const html = renderModal({ worktreeMode: "create_worktree" });
    expect(html).toContain("Hide Advanced Options");
    expect(html).toContain("Worktree mode");
    expect(html).toContain("Worktree repo path");
    expect(html).toContain("Worktree ref");
    expect(html).toContain("Auto-create under:");
    expect(html).toContain("Customize path");
    expect(html).not.toContain("Workdir (optional override)");
  });

  it("renders repo input only for reuse_worktree", () => {
    const html = renderModal({ worktreeMode: "reuse_worktree" });
    expect(html).toContain("Worktree repo path");
    expect(html).not.toContain("Worktree ref");
  });

  it("hides worktree inputs for use_existing", () => {
    const html = renderModal({ worktreeMode: "use_existing" });
    expect(html).toContain("Show Advanced Options");
    expect(html).not.toContain("Select worktree mode");
    expect(html).not.toContain("Worktree repo path");
    expect(html).not.toContain("Worktree ref");
  });

  it("can hide worktree advanced controls entirely", () => {
    const html = renderModal({
      worktreeMode: "create_worktree",
      showWorktreeAdvancedOptions: false,
    });
    expect(html).not.toContain("Show Advanced Options");
    expect(html).not.toContain("Hide Advanced Options");
    expect(html).not.toContain("Select worktree mode");
    expect(html).not.toContain("Worktree repo path");
    expect(html).not.toContain("Worktree ref");
  });

  it("renders error alert and guidance list when worktreeError is set", () => {
    const html = renderModal({ worktreeError: "Worktree missing" });
    expect(html).toContain("Worktree Setup Failed");
    expect(html).toContain("Worktree missing");
    expect(html).toContain("Check Safe Paths for the workdir and repo path.");
  });

  it("renders the preset command summary", () => {
    const html = renderModal({ agentPresetId: "gemini" as const });
    expect(html).toContain("Preset");
    expect(html).toContain("Gemini CLI");
    expect(html).toContain("Command");
    expect(html).toContain("gemini --acp");
    expect(html).toContain("Mode");
    expect(html).toContain("Chat");
  });

  it("renders custom workdir placeholder", () => {
    const html = renderModal({ workdirPlaceholder: "~/.agenthub/worktrees" });
    expect(html).toContain('placeholder="~/.agenthub/worktrees"');
  });

  it("shows editable workdir input in create_worktree mode when workdir was customized", () => {
    const html = renderModal({
      worktreeMode: "create_worktree",
      agentWorkdir: "/tmp/custom-agent-worktree",
    });
    expect(html).toContain("Workdir (optional override)");
    expect(html).toContain('value="/tmp/custom-agent-worktree"');
  });

  it("resolves preset id with fallback to default", () => {
    expect(resolveCreateAgentPresetId("gemini")).toBe("gemini");
    expect(resolveCreateAgentPresetId("invalid-preset")).toBe("codex");
    expect(resolveCreateAgentPresetId(null)).toBe("codex");
  });

  it("resolves worktree mode only for allowed values", () => {
    expect(resolveCreateAgentWorktreeMode("use_existing")).toBe("use_existing");
    expect(resolveCreateAgentWorktreeMode("create_worktree")).toBe("create_worktree");
    expect(resolveCreateAgentWorktreeMode("reuse_worktree")).toBe("reuse_worktree");
    expect(resolveCreateAgentWorktreeMode("invalid-mode")).toBeNull();
    expect(resolveCreateAgentWorktreeMode(null)).toBeNull();
  });

  it("auto-expands advanced options only when enabled and needed", () => {
    expect(shouldAutoExpandCreateAgentAdvancedOptions(true, "use_existing", null)).toBe(
      false
    );
    expect(shouldAutoExpandCreateAgentAdvancedOptions(true, "create_worktree", null)).toBe(
      true
    );
    expect(shouldAutoExpandCreateAgentAdvancedOptions(true, "use_existing", "err")).toBe(
      true
    );
    expect(shouldAutoExpandCreateAgentAdvancedOptions(false, "create_worktree", "err")).toBe(
      false
    );
  });
});
