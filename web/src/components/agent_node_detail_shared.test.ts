import { describe, expect, it } from "vitest";

import type { AgentNodeRecord, AgentRecord } from "../api";
import {
  deriveDetectedNodeRuntimes,
  resolveAgentRuntimeLabels,
} from "./agent_node_detail_shared";

const remoteNode: AgentNodeRecord = {
  id: "node-east",
  name: "Node East",
  grpc_target: "https://node-east.internal:50051",
  tls_server_name: "node-east.internal",
  default_worktree_root: "~/.agenthub/worktrees/node-east",
  last_seen_at: null,
  is_main: false,
  created_at: 1,
  updated_at: 1,
};

function agent(overrides: Partial<AgentRecord>): AgentRecord {
  return {
    id: "agent-1",
    name: "Worker A",
    command: "agenthub codex",
    args: [],
    workdir: "/tmp/worker-a",
    status: "running",
    target_node_id: "node-east",
    worktree_mode: "use_existing",
    code_mode: false,
    created_at: 1,
    updated_at: 1,
    ...overrides,
  };
}

describe("deriveDetectedNodeRuntimes", () => {
  it("returns runtime tags in a stable product order", () => {
    const runtimes = deriveDetectedNodeRuntimes(remoteNode, [
      agent({ command: "gemini" }),
      agent({ id: "agent-2", command: "agenthub-acp", args: ["codex"] }),
      agent({ id: "agent-3", command: "agenthub-acp", args: ["claude"] }),
    ]);

    expect(runtimes).toEqual([
      { label: "AgentHub Runtime", available: true },
      { label: "Codex CLI", available: true },
      { label: "Gemini CLI", available: true },
      { label: "Claude ACP", available: true },
    ]);
  });

  it("tolerates missing commands and keeps unobserved attached-agent markers", () => {
    const runtimes = deriveDetectedNodeRuntimes(remoteNode, [
      agent({ command: undefined as unknown as string }),
    ]);

    expect(runtimes).toEqual([
      { label: "Codex CLI (no attached agent observed)", available: false },
      { label: "Gemini CLI (no attached agent observed)", available: false },
      { label: "Claude ACP (no attached agent observed)", available: false },
    ]);
  });
});

describe("resolveAgentRuntimeLabels", () => {
  it("keeps stable runtime badges for agenthub and codex-backed agents", () => {
    expect(resolveAgentRuntimeLabels(agent({ command: "agenthub codex" }))).toEqual([
      { label: "AgentHub Runtime", tone: "outline" },
      { label: "Codex CLI", tone: "subtle" },
    ]);
    expect(
      resolveAgentRuntimeLabels(agent({ command: "agenthub-acp", args: ["codex"] }))
    ).toEqual([
      { label: "AgentHub Runtime", tone: "outline" },
      { label: "Codex CLI", tone: "subtle" },
    ]);
    expect(resolveAgentRuntimeLabels(agent({ command: "codex-acp" }))).toEqual([
      { label: "Codex CLI", tone: "subtle" },
    ]);
  });

  it("labels Claude ACP runtimes", () => {
    expect(
      resolveAgentRuntimeLabels(agent({ command: "agenthub-acp", args: ["claude"] }))
    ).toContainEqual({ label: "AgentHub Runtime", tone: "outline" });
    expect(
      resolveAgentRuntimeLabels(agent({ command: "agenthub-acp", args: ["claude"] }))
    ).toContainEqual({ label: "Claude ACP", tone: "subtle" });
    expect(
      resolveAgentRuntimeLabels(agent({ command: "claude-agent-acp" }))
    ).toEqual([{ label: "Claude ACP", tone: "subtle" }]);
    expect(
      resolveAgentRuntimeLabels(
        agent({ command: "claude-code-acp-rs", args: ["--acp"] })
      )
    ).toEqual([{ label: "Claude ACP", tone: "subtle" }]);
  });

  it("does not label non-ACP Claude Code Rust invocations as ACP", () => {
    expect(
      resolveAgentRuntimeLabels(agent({ command: "claude-code-acp-rs", args: [] }))
    ).toEqual([{ label: "Custom Runtime", tone: "outline" }]);
  });

  it("falls back to a custom runtime label when no known provider is detected", () => {
    expect(
      resolveAgentRuntimeLabels(
        agent({
          command: "python worker.py",
          code_mode: false,
        })
      )
    ).toEqual([{ label: "Custom Runtime", tone: "outline" }]);
  });
});
