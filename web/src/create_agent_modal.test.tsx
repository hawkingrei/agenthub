import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
import { CreateAgentModal } from "./components/create_agent_modal";

const baseProps = {
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
  withinPortal: false,
  onCreateAgent: () => {},
  onClose: () => {},
};

const renderModal = (overrides?: Partial<typeof baseProps>) =>
  renderToStaticMarkup(
    <MantineProvider>
      <CreateAgentModal {...baseProps} {...overrides} />
    </MantineProvider>
  );

describe("CreateAgentModal", () => {
  it("renders repo and ref inputs for create_worktree", () => {
    const html = renderModal({ worktreeMode: "create_worktree" });
    expect(html).toContain("Worktree repo path");
    expect(html).toContain("Worktree ref");
  });

  it("renders repo input only for reuse_worktree", () => {
    const html = renderModal({ worktreeMode: "reuse_worktree" });
    expect(html).toContain("Worktree repo path");
    expect(html).not.toContain("Worktree ref");
  });

  it("hides worktree inputs for use_existing", () => {
    const html = renderModal({ worktreeMode: "use_existing" });
    expect(html).not.toContain("Worktree repo path");
    expect(html).not.toContain("Worktree ref");
  });

  it("renders error alert and guidance list when worktreeError is set", () => {
    const html = renderModal({ worktreeError: "Worktree missing" });
    expect(html).toContain("Worktree Setup Failed");
    expect(html).toContain("Worktree missing");
    expect(html).toContain("Check Safe Paths for the workdir and repo path.");
  });
});
