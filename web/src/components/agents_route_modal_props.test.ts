import { describe, expect, it, vi } from "vitest";
import {
  buildAgentNodeSectionProps,
  buildCreateAgentModalProps,
  buildPermissionModalProps,
} from "./agents_route_modal_props";
import { AgentNodeSectionProps } from "./agent_node_section";
import { CreateAgentModalProps } from "./create_agent_modal";
import { PermissionModalProps } from "./permission_modal";

const baseCreateAgentModalProps: CreateAgentModalProps = {
  agentName: "",
  setAgentName: vi.fn(),
  agentWorkdir: "",
  setAgentWorkdir: vi.fn(),
  agentPresetId: "codex_default",
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
  createBusy: false,
  onCreateAgent: vi.fn(),
  onClose: vi.fn(),
};

const baseAgentNodeSectionProps: AgentNodeSectionProps = {
  nodes: [],
  agents: [],
  nodeJoinBootstrap: null,
  targetNodeId: "main",
  onTargetNodeIdChange: vi.fn(),
  nodeIdInput: "",
  onNodeIdInputChange: vi.fn(),
  nodeNameInput: "",
  onNodeNameInputChange: vi.fn(),
  grpcTargetInput: "",
  onGrpcTargetInputChange: vi.fn(),
  tlsServerNameInput: "",
  onTlsServerNameInputChange: vi.fn(),
  defaultWorktreeRootInput: "",
  onDefaultWorktreeRootInputChange: vi.fn(),
  createBusy: false,
  updatingNodeIds: {},
  deletingNodeIds: {},
  onCreateNode: vi.fn(),
  onUpdateNode: vi.fn(),
  onDeleteNode: vi.fn(),
};

const basePermissionModalProps: PermissionModalProps = {
  permissions: [],
  permissionBusy: null,
  onRespond: vi.fn(),
};

describe("agents route modal props helpers", () => {
  it("keeps create-agent modal props unchanged", () => {
    expect(buildCreateAgentModalProps(baseCreateAgentModalProps)).toBe(
      baseCreateAgentModalProps
    );
  });

  it("passes through optional node-section props", () => {
    expect(buildAgentNodeSectionProps(baseAgentNodeSectionProps)).toBe(
      baseAgentNodeSectionProps
    );
    expect(buildAgentNodeSectionProps(null)).toBeNull();
  });

  it("passes through optional permission-modal props", () => {
    expect(buildPermissionModalProps(basePermissionModalProps)).toBe(
      basePermissionModalProps
    );
    expect(buildPermissionModalProps(null)).toBeNull();
  });
});
