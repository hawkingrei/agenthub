import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
import {
  CreateAgentModal,
  CODEX_RUNTIME_MODEL_OPTIONS,
  listCodexRuntimeModelOptions,
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
    expect(html).toContain("Hide Advanced workspace options");
    expect(html).toContain("Workspace mode");
    expect(html).toContain("Repository path");
    expect(html).toContain("Repository ref");
    expect(html).toContain("Auto-create under:");
    expect(html).toContain("Customize path");
    expect(html).not.toContain("Workdir (optional override)");
  });

  it("renders repo input only for reuse_worktree", () => {
    const html = renderModal({ worktreeMode: "reuse_worktree" });
    expect(html).toContain("Repository path");
    expect(html).not.toContain("Repository ref");
  });

  it("hides worktree inputs for use_existing", () => {
    const html = renderModal({ worktreeMode: "use_existing" });
    expect(html).toContain("Show Advanced workspace options");
    expect(html).not.toContain("Select worktree mode");
    expect(html).not.toContain("Repository path");
    expect(html).not.toContain("Repository ref");
  });

  it("can hide worktree advanced controls entirely", () => {
    const html = renderModal({
      worktreeMode: "create_worktree",
      showWorktreeAdvancedOptions: false,
    });
    expect(html).not.toContain("Show Advanced workspace options");
    expect(html).not.toContain("Hide Advanced workspace options");
    expect(html).not.toContain("Select worktree mode");
    expect(html).not.toContain("Repository path");
    expect(html).not.toContain("Repository ref");
  });

  it("renders error alert and guidance list when worktreeError is set", () => {
    const html = renderModal({ worktreeError: "Worktree missing" });
    expect(html).toContain("Worktree Setup Failed");
    expect(html).toContain("Worktree missing");
    expect(html).toContain("Check Safe Paths for the workdir and repo path.");
  });

  it("renders a plain validation alert inside the modal when formError is set", () => {
    // Regression test: submit-time validation errors (e.g. "workdir is required") used to only
    // reach a page-level banner that this always-on-top, non-dismissible modal covers, so the user
    // saw no feedback at all. The message must render inside the modal itself.
    const html = renderModal({ formError: "workdir is required" });
    expect(html).toContain("workdir is required");
  });

  it("renders no validation alert when formError is not set", () => {
    const html = renderModal({ formError: null });
    expect(html).not.toContain("workdir is required");
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

  it("shows runtime profile controls only for Codex and Claude presets", () => {
    expect(renderModal()).toContain("Runtime model");
    expect(renderModal()).toContain("Thinking level");
    expect(renderModal({ agentPresetId: "claude" })).toContain("Runtime model");
    expect(renderModal({ agentPresetId: "gemini" })).not.toContain("Runtime model");
  });

  it("renders the Codex runtime model selector with bundled model presets", () => {
    const html = renderModal();

    expect(html).toContain("Runtime model");
    expect(html).toContain("Provider default");
    expect(CODEX_RUNTIME_MODEL_OPTIONS).toEqual([
      { value: "gpt-5.6-sol", label: "GPT-5.6-Sol" },
      { value: "gpt-5.6-terra", label: "GPT-5.6-Terra" },
      { value: "gpt-5.6-luna", label: "GPT-5.6-Luna" },
      { value: "gpt-5.5", label: "GPT-5.5" },
      { value: "gpt-5.4", label: "GPT-5.4" },
      { value: "gpt-5.4-mini", label: "GPT-5.4-Mini" },
      { value: "gpt-5.2", label: "GPT-5.2" },
    ]);
  });

  it("preserves a custom Codex runtime model in the selector", () => {
    expect(listCodexRuntimeModelOptions("gpt-5.5")).toEqual(
      CODEX_RUNTIME_MODEL_OPTIONS
    );
    expect(listCodexRuntimeModelOptions("custom-codex-model")).toEqual([
      ...CODEX_RUNTIME_MODEL_OPTIONS,
      { value: "custom-codex-model", label: "custom-codex-model" },
    ]);
  });

  it("can hide the command summary strip", () => {
    const html = renderModal({ showCommandSummary: false });
    expect(html).not.toContain("Command");
    expect(html).not.toContain("agenthub-codex-acp");
    expect(html).toContain("Mode");
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
