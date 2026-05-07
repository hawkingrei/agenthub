import React from "react";
import { Button, Stack, Text, TextInput } from "@mantine/core";
import {
  buildTeamDetailPath,
  buildTeamWorkspacePath,
  navigateToPath,
  shouldHandleInAppLinkClick,
} from "../app_route_selection";
import { isAgentActiveStatus } from "../agent_ws";
import type { AgentNodeUpdate, TeamDefinitionRecord } from "../api";
import {
  AgentNodeJoinBootstrapInfo,
  AgentNodeRecord,
  AgentRecord,
} from "../api";
import { Badge, EmptyState, SelectableListItem } from "../ui/primitives";
import {
  AgentNodeDetailCard,
  resolveAvailableNodes,
  resolveNodeRoleLabel,
} from "./agent_node_detail_shared";
import { validateAgentNodeUpdateDraft } from "./agent_node_validation";
import {
  SECTION_CARD_CLASS,
  SECTION_HEADER_CLASS,
  WORKSPACE_PANEL_ROOT_CLASS,
} from "../ui/tailwind_classes";

const NODE_ROSTER_ITEM_CLASS =
  "w-full rounded-xl border-2 border-transparent bg-white/70 px-2.5 py-2.5 shadow-none hover:border-black hover:bg-white";
const NODE_ROSTER_ITEM_ACTIVE_CLASS = "border-black bg-white";
const NODE_TEAM_LINK_CLASS =
  "text-notion-text underline decoration-transparent underline-offset-2 transition hover:decoration-current";
const NODE_MEMBER_DRILLDOWN_CLASS =
  "flex min-w-0 max-w-full items-center justify-between gap-2 rounded-2xl border border-ui-border/80 bg-white/92 px-2.5 py-1.5 text-[11px] font-medium text-notion-text shadow-notion-row";
const NODE_MEMBER_ACTION_CLASS =
  "rounded-full px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-[0.04em] text-notion-text-muted transition hover:bg-black/5 hover:text-notion-text";
const NODE_TEAM_CARD_CLASS =
  "rounded-2xl border border-ui-border/70 bg-[linear-gradient(180deg,rgba(255,255,255,0.98),rgba(249,250,251,0.88))] px-3.5 py-3.5 shadow-[0_8px_24px_rgba(15,23,42,0.05)]";
const NODE_TEAM_SUMMARY_CARD_CLASS =
  "rounded-2xl border border-ui-border/75 bg-[linear-gradient(180deg,rgba(255,255,255,0.98),rgba(248,250,252,0.9))] px-4 py-4 shadow-[0_10px_28px_rgba(15,23,42,0.05)]";
const NODE_TEAM_METRIC_ITEM_CLASS =
  "rounded-xl border border-ui-border/70 bg-white/80 px-2.5 py-2 text-left shadow-[0_1px_2px_rgba(15,23,42,0.03)]";
const NODE_TEAM_METRIC_LABEL_CLASS =
  "text-[10px] font-bold uppercase tracking-[0.08em] text-notion-text-muted/80";
const NODE_TEAM_METRIC_VALUE_CLASS = "mt-1 text-[13px] font-semibold text-notion-text";

type NodeTeamUsageSummary = {
  teamId: string;
  teamName: string;
  matchedMembers: Array<{
    memberId: string;
    label: string;
    role: string | null;
  }>;
  activeAgentCount: number;
};

export type NodeEditDraft = {
  name: string;
  grpcTarget: string;
  tlsServerName: string;
  defaultWorktreeRoot: string;
};

function pluralize(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

export function buildNodeEditDraft(node: AgentNodeRecord): NodeEditDraft {
  return {
    name: node.name,
    grpcTarget: node.grpc_target ?? "",
    tlsServerName: node.tls_server_name ?? "",
    defaultWorktreeRoot: node.default_worktree_root ?? "",
  };
}

export function buildNodeNameUpdatePayload(
  node: AgentNodeRecord,
  draft: NodeEditDraft
): AgentNodeUpdate | null {
  const grpcTarget = node.grpc_target?.trim() || "";
  if (!grpcTarget) {
    return null;
  }
  return {
    name: draft.name.trim(),
    grpc_target: grpcTarget,
    tls_server_name: node.tls_server_name?.trim() || null,
    default_worktree_root: node.default_worktree_root?.trim() || null,
  };
}

export function buildNodeSettingsUpdatePayload(
  node: AgentNodeRecord,
  draft: Pick<NodeEditDraft, "grpcTarget" | "tlsServerName" | "defaultWorktreeRoot">
): AgentNodeUpdate {
  return {
    name: node.name.trim(),
    grpc_target: draft.grpcTarget.trim(),
    tls_server_name: draft.tlsServerName.trim() || null,
    default_worktree_root: draft.defaultWorktreeRoot.trim() || null,
  };
}

function resolveNameUpdateError(node: AgentNodeRecord, draft: NodeEditDraft): string | null {
  if (!draft.name.trim()) {
    return "Node name is required.";
  }
  if (!node.grpc_target?.trim()) {
    return "This node is missing a persisted gRPC target.";
  }
  return null;
}

function parseTeamSpecMembers(
  spec: unknown
): Array<{ memberId: string; role: string | null }> {
  if (!spec || typeof spec !== "object") {
    return [];
  }
  const members = (spec as { members?: unknown }).members;
  if (!Array.isArray(members)) {
    return [];
  }
  return members
    .map((member) => {
      if (!member || typeof member !== "object") {
        return null;
      }
      const memberId = (member as { member_id?: unknown }).member_id;
      const role = (member as { role?: unknown }).role;
      const normalizedMemberId = typeof memberId === "string" ? memberId.trim() : "";
      if (!normalizedMemberId) {
        return null;
      }
      return {
        memberId: normalizedMemberId,
        role: typeof role === "string" ? role.trim() || null : null,
      };
    })
    .filter(
      (member): member is { memberId: string; role: string | null } => Boolean(member)
    );
}

function resolveAgentNodeMembership(agent: AgentRecord): string {
  return agent.target_node_id?.trim() || "main";
}

function deriveNodeTeamUsageSummaries(
  teams: TeamDefinitionRecord[],
  agents: AgentRecord[],
  fallbackAgentsById: Record<string, AgentRecord | null>,
  nodeId: string
): NodeTeamUsageSummary[] {
  const agentsById = new Map(agents.map((agent) => [agent.id, agent]));
  for (const [memberId, agent] of Object.entries(fallbackAgentsById)) {
    if (agent && !agentsById.has(memberId)) {
      agentsById.set(memberId, agent);
    }
  }
  return teams
    .map((team) => {
      const matchedMembers = parseTeamSpecMembers(team.spec)
        .map((member) => {
          const agent = agentsById.get(member.memberId);
          if (!agent || resolveAgentNodeMembership(agent) !== nodeId) {
            return null;
          }
          return {
            memberId: member.memberId,
            label: agent.name?.trim() || member.memberId,
            role: member.role,
          };
        })
        .filter(
          (
            member
          ): member is {
            memberId: string;
            label: string;
            role: string | null;
          } => Boolean(member)
        );
      if (matchedMembers.length === 0) {
        return null;
      }
      const activeAgentCount = matchedMembers.filter((memberId) => {
        const status = agentsById.get(memberId.memberId)?.status ?? "";
        return isAgentActiveStatus(status);
      }).length;
      return {
        teamId: team.id,
        teamName: team.name,
        matchedMembers,
        activeAgentCount,
      };
    })
    .filter((summary): summary is NodeTeamUsageSummary => Boolean(summary))
    .sort((a, b) => {
      if (b.matchedMembers.length !== a.matchedMembers.length) {
        return b.matchedMembers.length - a.matchedMembers.length;
      }
      return a.teamName.localeCompare(b.teamName);
    });
}

function handleInAppLinkClick(
  event: React.MouseEvent<HTMLAnchorElement>,
  pathname: string
): void {
  if (!shouldHandleInAppLinkClick(event)) {
    return;
  }
  event.preventDefault();
  navigateToPath(pathname);
}

function buildTeamMemberAcpPath(teamId: string, memberId: string): string {
  return buildTeamWorkspacePath(teamId, "members", null, null, memberId, "agent_acp");
}

function buildTeamMemberConsolePath(teamId: string, memberId: string): string {
  return buildTeamWorkspacePath(teamId, "members", null, null, memberId, "member_console");
}

type AgentNodesWorkbenchProps = {
  nodes: AgentNodeRecord[];
  agents: AgentRecord[];
  teams?: TeamDefinitionRecord[];
  teamMemberAgentsById?: Record<string, AgentRecord | null>;
  selectedNodeId: string;
  nodeJoinBootstrap: AgentNodeJoinBootstrapInfo | null;
  nodeJoinBootstrapLoading: boolean;
  nodeJoinBootstrapError: string | null;
  updatingNodeIds?: Record<string, boolean>;
  deletingNodeIds?: Record<string, boolean>;
  onSelectNode: (nodeId: string) => void;
  onOpenAgent: (agentId: string) => void;
  onCreateAgent?: () => void;
  onUpdateNode?: (nodeId: string, payload: AgentNodeUpdate) => void;
  onDeleteNode?: (nodeId: string) => void;
};

export function AgentNodesWorkbench({
  nodes,
  agents,
  teams = [],
  teamMemberAgentsById = {},
  selectedNodeId,
  nodeJoinBootstrap,
  nodeJoinBootstrapLoading,
  nodeJoinBootstrapError,
  updatingNodeIds = {},
  deletingNodeIds = {},
  onSelectNode,
  onOpenAgent,
  onCreateAgent,
  onUpdateNode,
  onDeleteNode,
}: AgentNodesWorkbenchProps) {
  const availableNodes = React.useMemo(() => resolveAvailableNodes(nodes), [nodes]);
  const agentsByNodeId = React.useMemo(() => {
    const buckets = new Map<string, AgentRecord[]>();
    for (const agent of agents) {
      const nodeId = agent.target_node_id?.trim() || "main";
      const current = buckets.get(nodeId) ?? [];
      current.push(agent);
      buckets.set(nodeId, current);
    }
    return buckets;
  }, [agents]);
  const teamUsageByNodeId = React.useMemo(() => {
    const summaries = new Map<string, NodeTeamUsageSummary[]>();
    for (const node of availableNodes) {
      summaries.set(
        node.id,
        deriveNodeTeamUsageSummaries(teams, agents, teamMemberAgentsById, node.id)
      );
    }
    return summaries;
  }, [agents, availableNodes, teamMemberAgentsById, teams]);
  const effectiveSelectedNodeId =
    selectedNodeId.trim() ||
    availableNodes.find((node) => node.is_main)?.id ||
    availableNodes[0]?.id ||
    "";
  const selectedNode =
    availableNodes.find((node) => node.id === effectiveSelectedNodeId) ?? null;
  const selectedNodeAgents = selectedNode ? (agentsByNodeId.get(selectedNode.id) ?? []) : [];
  const selectedNodeTeams = React.useMemo(
    () => (selectedNode ? (teamUsageByNodeId.get(selectedNode.id) ?? []) : []),
    [selectedNode, teamUsageByNodeId]
  );
  const selectedNodeTeamMemberCount = selectedNodeTeams.reduce(
    (sum, team) => sum + team.matchedMembers.length,
    0
  );
  const selectedNodeActiveTeamAgentCount = selectedNodeTeams.reduce(
    (sum, team) => sum + team.activeAgentCount,
    0
  );
  const selectedNodeCoordinatorCount = selectedNodeTeams.reduce(
    (sum, team) =>
      sum + team.matchedMembers.filter((member) => member.role === "coordinator").length,
    0
  );
  const selectedNodeWorkerCount = selectedNodeTeams.reduce(
    (sum, team) =>
      sum + team.matchedMembers.filter((member) => member.role === "worker").length,
    0
  );
  const [editDrafts, setEditDrafts] = React.useState<
    Record<string, NodeEditDraft>
  >({});

  React.useEffect(() => {
    setEditDrafts((prev) => {
      const next: typeof prev = {};
      for (const node of availableNodes) {
        if (node.is_main) {
          continue;
        }
        next[node.id] = prev[node.id] ?? buildNodeEditDraft(node);
      }
      return next;
    });
  }, [availableNodes]);

  const handleSaveNodeName = React.useCallback(
    (nodeId: string, draft: NodeEditDraft) => {
      const node = availableNodes.find((candidate) => candidate.id === nodeId);
      if (!node) {
        return;
      }
      const payload = buildNodeNameUpdatePayload(node, draft);
      if (!payload) {
        return;
      }
      onUpdateNode?.(nodeId, payload);
    },
    [availableNodes, onUpdateNode]
  );

  const handleSaveNodeSettings = React.useCallback(
    (
      nodeId: string,
      draft: {
        grpcTarget: string;
        tlsServerName: string;
        defaultWorktreeRoot: string;
      }
    ) => {
      const node = availableNodes.find((candidate) => candidate.id === nodeId);
      if (!node) {
        return;
      }
      onUpdateNode?.(nodeId, buildNodeSettingsUpdatePayload(node, draft));
    },
    [availableNodes, onUpdateNode]
  );

  if (!selectedNode) {
    return (
      <div className={WORKSPACE_PANEL_ROOT_CLASS}>
        <EmptyState
          title="No nodes registered yet"
          body="Register a remote node or use the local control plane node to start routing agents."
          className="border border-dashed border-ui-border bg-white/80 px-4 py-8"
        />
      </div>
    );
  }

  return (
    <div className={WORKSPACE_PANEL_ROOT_CLASS}>
      <div className="mx-auto flex w-full max-w-6xl min-h-full flex-col gap-4">
        <div>
          <Text size="xl" fw={700}>
            Node Detail
          </Text>
          <Text size="sm" c="dimmed" mt={4}>
            Inspect node routing metadata, copy the connect command, and trace which teams currently
            depend on this global node.
          </Text>
        </div>

        <div
          className="grid gap-4 lg:grid-cols-[240px_minmax(0,1fr)]"
          data-node-detail-layout="true"
        >
          <div className="lg:sticky lg:top-4 lg:self-start">
            <div
              className="rounded-2xl border border-ui-border bg-ui-surface-soft/70 p-3 shadow-sm"
              data-node-roster="true"
            >
              <Stack gap="xs">
                <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                  Nodes
                </Text>
                <Text size="xs" c="dimmed">
                  Global machine roster
                </Text>
                {availableNodes.map((node) => {
                  const nodeAgents = agentsByNodeId.get(node.id) ?? [];
                  const nodeTeams = teamUsageByNodeId.get(node.id) ?? [];
                  const isSelected = node.id === selectedNode.id;
                  return (
                    <SelectableListItem
                      key={node.id}
                      aria-pressed={isSelected}
                      active={isSelected}
                      className={[
                        NODE_ROSTER_ITEM_CLASS,
                        isSelected ? NODE_ROSTER_ITEM_ACTIVE_CLASS : "",
                      ].join(" ")}
                      onClick={() => onSelectNode(node.id)}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <Text size="sm" fw={600}>
                              {node.name}
                            </Text>
                            <Badge tone={node.is_main ? "subtle" : "outline"} className="uppercase">
                              {resolveNodeRoleLabel(node)}
                            </Badge>
                          </div>
                          <Text size="xs" c="dimmed" mt={4}>
                            {nodeAgents.length} agent{nodeAgents.length === 1 ? "" : "s"} ·{" "}
                            {nodeTeams.length} team{nodeTeams.length === 1 ? "" : "s"}
                          </Text>
                        </div>
                      </div>
                    </SelectableListItem>
                  );
                })}
              </Stack>
            </div>
          </div>

          <div className="min-w-0 space-y-4">
            <AgentNodeDetailCard
              node={selectedNode}
              agents={selectedNodeAgents}
              nodeJoinBootstrap={nodeJoinBootstrap}
              nodeJoinBootstrapLoading={nodeJoinBootstrapLoading}
              nodeJoinBootstrapError={nodeJoinBootstrapError}
              onOpenAgent={onOpenAgent}
              onCreateAgent={onCreateAgent}
            />
            {selectedNode.is_main ? (
              <>
                <div className={SECTION_CARD_CLASS}>
                  <Stack gap="sm">
                    <div>
                      <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                        Name
                      </Text>
                      <Text size="sm" c="dimmed" mt={6}>
                        Canonical display name for the local control-plane node.
                      </Text>
                    </div>
                    <div className="rounded-xl border border-ui-border/75 bg-white/80 px-3 py-3">
                      <Text size="sm" fw={600}>
                        {selectedNode.name}
                      </Text>
                      <Text size="xs" c="dimmed" mt={6}>
                        The local control-plane node name is currently read only from this surface.
                      </Text>
                    </div>
                  </Stack>
                </div>
                <div className={SECTION_CARD_CLASS}>
                  <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                    Danger Zone
                  </Text>
                  <Text size="sm" c="dimmed" mt={8}>
                    The local control-plane node cannot be deleted or re-pointed from this surface.
                  </Text>
                </div>
              </>
            ) : (
              <div className="space-y-4">
                <div className={SECTION_CARD_CLASS}>
                  <Stack gap="sm">
                    <div>
                      <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                        Name
                      </Text>
                      <Text size="sm" c="dimmed" mt={6}>
                        Keep the node display name visible and editable as a first-class object field.
                      </Text>
                    </div>
                    {(() => {
                      const draft = editDrafts[selectedNode.id] ?? buildNodeEditDraft(selectedNode);
                      const nameError = resolveNameUpdateError(selectedNode, draft);
                      return (
                        <>
                          <TextInput
                            label="Node name"
                            value={draft.name}
                            onChange={(event) =>
                              setEditDrafts((prev) => ({
                                ...prev,
                                [selectedNode.id]: {
                                  ...draft,
                                  name: event.currentTarget.value,
                                },
                              }))
                            }
                          />
                          <div className="flex flex-wrap items-center justify-between gap-3">
                            <Text size="xs" c={nameError ? "red" : "dimmed"}>
                              {nameError ??
                                "This is the canonical operator-facing name shown in global node rosters and detail pages."}
                            </Text>
                            <Button
                              variant="light"
                              size="xs"
                              loading={Boolean(updatingNodeIds[selectedNode.id])}
                              disabled={
                                Boolean(updatingNodeIds[selectedNode.id]) || nameError !== null
                              }
                              onClick={() => handleSaveNodeName(selectedNode.id, draft)}
                            >
                              Save Name
                            </Button>
                          </div>
                        </>
                      );
                    })()}
                  </Stack>
                </div>

                <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_280px]">
                  <div className={SECTION_CARD_CLASS}>
                    <Stack gap="sm">
                      <div>
                        <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                          Settings
                        </Text>
                        <Text size="sm" c="dimmed" mt={6}>
                          Update routing and default worktree behavior for this node without mixing
                          in the object name field.
                        </Text>
                      </div>
                      {(() => {
                        const draft =
                          editDrafts[selectedNode.id] ?? buildNodeEditDraft(selectedNode);
                        const updateError = validateAgentNodeUpdateDraft({
                          nodeName: selectedNode.name,
                          grpcTarget: draft.grpcTarget,
                        });
                        return (
                          <>
                            <TextInput
                              label="gRPC target"
                              value={draft.grpcTarget}
                              onChange={(event) =>
                                setEditDrafts((prev) => ({
                                  ...prev,
                                  [selectedNode.id]: {
                                    ...draft,
                                    grpcTarget: event.currentTarget.value,
                                  },
                                }))
                              }
                            />
                            <div className="grid gap-3 lg:grid-cols-2">
                              <TextInput
                                label="TLS server name"
                                value={draft.tlsServerName}
                                onChange={(event) =>
                                  setEditDrafts((prev) => ({
                                    ...prev,
                                    [selectedNode.id]: {
                                      ...draft,
                                      tlsServerName: event.currentTarget.value,
                                    },
                                  }))
                                }
                              />
                              <TextInput
                                label="Default worktree root"
                                placeholder="Optional"
                                value={draft.defaultWorktreeRoot}
                                onChange={(event) =>
                                  setEditDrafts((prev) => ({
                                    ...prev,
                                    [selectedNode.id]: {
                                      ...draft,
                                      defaultWorktreeRoot: event.currentTarget.value,
                                    },
                                  }))
                                }
                              />
                            </div>
                            <div className="flex flex-wrap items-center justify-between gap-3">
                              <Text size="xs" c={updateError ? "red" : "dimmed"}>
                                {updateError ??
                                  "Leave Default worktree root blank to require explicit remote workdir."}
                              </Text>
                              <Button
                                variant="light"
                                size="xs"
                                loading={Boolean(updatingNodeIds[selectedNode.id])}
                                disabled={
                                  Boolean(updatingNodeIds[selectedNode.id]) ||
                                  updateError !== null
                                }
                                onClick={() =>
                                  handleSaveNodeSettings(selectedNode.id, {
                                    grpcTarget: draft.grpcTarget,
                                    tlsServerName: draft.tlsServerName,
                                    defaultWorktreeRoot: draft.defaultWorktreeRoot,
                                  })
                                }
                              >
                                Save Settings
                              </Button>
                            </div>
                          </>
                        );
                      })()}
                    </Stack>
                  </div>

                  <div className="rounded-xl border border-red-200 bg-red-50/70 px-4 py-4">
                    <Stack gap="sm">
                      <div>
                        <Text size="xs" fw={700} c="red" className={SECTION_HEADER_CLASS}>
                          Danger Zone
                        </Text>
                        <Text size="sm" c="dimmed" mt={6}>
                          Delete this node only after its attached agents have been removed or rerouted.
                        </Text>
                      </div>
                      <Text size="xs" c={selectedNodeAgents.length > 0 ? "red" : "dimmed"}>
                        {selectedNodeAgents.length > 0
                          ? `This node still has ${selectedNodeAgents.length} attached agent${selectedNodeAgents.length === 1 ? "" : "s"}.`
                          : "No attached agents remain on this node."}
                      </Text>
                      <Button
                        color="red"
                        variant="light"
                        size="xs"
                        loading={Boolean(deletingNodeIds[selectedNode.id])}
                        disabled={
                          Boolean(deletingNodeIds[selectedNode.id]) || selectedNodeAgents.length > 0
                        }
                        onClick={() => onDeleteNode?.(selectedNode.id)}
                      >
                        Delete Node
                      </Button>
                    </Stack>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>

        <div className={SECTION_CARD_CLASS}>
          <div className={NODE_TEAM_SUMMARY_CARD_CLASS}>
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <div className="inline-flex h-8 w-8 items-center justify-center rounded-2xl border border-ui-border/70 bg-white/85 text-notion-text-muted shadow-notion-row">
                    <i className="bi bi-diagram-3 text-[13px]" aria-hidden="true" />
                  </div>
                  <div>
                    <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                      Teams Using This Node
                    </Text>
                    <Text size="sm" fw={600} mt={2}>
                      Global team attachment map
                    </Text>
                  </div>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone="outline">
                  {selectedNodeTeams.length} team{selectedNodeTeams.length === 1 ? "" : "s"}
                </Badge>
                <Badge tone="outline">
                  {pluralize(selectedNodeTeamMemberCount, "member")}
                </Badge>
                <Badge tone="subtle">
                  {selectedNodeActiveTeamAgentCount} active
                </Badge>
              </div>
            </div>
            <Text size="sm" c="dimmed" mt={10}>
              Because nodes are global resources, this section shows which teams currently land
              members on the selected node rather than treating node usage as team-local state.
            </Text>
            <div
              className="mt-3 grid gap-2 min-[420px]:grid-cols-2 sm:grid-cols-4"
              data-node-team-summary-metrics="true"
            >
              <div className={NODE_TEAM_METRIC_ITEM_CLASS}>
                <div className={NODE_TEAM_METRIC_LABEL_CLASS}>Teams</div>
                <div className={NODE_TEAM_METRIC_VALUE_CLASS}>{selectedNodeTeams.length}</div>
              </div>
              <div className={NODE_TEAM_METRIC_ITEM_CLASS}>
                <div className={NODE_TEAM_METRIC_LABEL_CLASS}>Members</div>
                <div className={NODE_TEAM_METRIC_VALUE_CLASS}>{selectedNodeTeamMemberCount}</div>
              </div>
              <div className={NODE_TEAM_METRIC_ITEM_CLASS}>
                <div className={NODE_TEAM_METRIC_LABEL_CLASS}>Coordinators</div>
                <div className={NODE_TEAM_METRIC_VALUE_CLASS}>
                  {selectedNodeCoordinatorCount}
                </div>
              </div>
              <div className={NODE_TEAM_METRIC_ITEM_CLASS}>
                <div className={NODE_TEAM_METRIC_LABEL_CLASS}>Workers</div>
                <div className={NODE_TEAM_METRIC_VALUE_CLASS}>{selectedNodeWorkerCount}</div>
              </div>
            </div>
          </div>
          {selectedNodeTeams.length > 0 ? (
            <div className="mt-3 grid gap-3">
              {selectedNodeTeams.map((team) => (
                <div key={team.teamId} className={NODE_TEAM_CARD_CLASS}>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <a
                          href={buildTeamDetailPath(team.teamId)}
                          className={NODE_TEAM_LINK_CLASS}
                          title={`Open team detail for ${team.teamName}`}
                          onClick={(event) =>
                            handleInAppLinkClick(event, buildTeamDetailPath(team.teamId))
                          }
                        >
                          <Text size="sm" fw={700} span inherit>
                            {team.teamName}
                          </Text>
                        </a>
                        <Badge tone="outline">
                          {pluralize(team.matchedMembers.length, "member")}
                        </Badge>
                        <Badge tone="subtle">{team.activeAgentCount} active</Badge>
                      </div>
                      <Text size="xs" c="dimmed" mt={4}>
                        <a
                          href={buildTeamDetailPath(team.teamId)}
                          className={NODE_TEAM_LINK_CLASS}
                          title={`Open team detail for ${team.teamId}`}
                          onClick={(event) =>
                            handleInAppLinkClick(event, buildTeamDetailPath(team.teamId))
                          }
                        >
                          {`team_id=${team.teamId}`}
                        </a>
                      </Text>
                    </div>
                    <div className="grid min-w-0 w-full gap-2 min-[420px]:grid-cols-2 sm:max-w-[280px] sm:grid-cols-3">
                      <div className={NODE_TEAM_METRIC_ITEM_CLASS}>
                        <div className={NODE_TEAM_METRIC_LABEL_CLASS}>Members</div>
                        <div className={NODE_TEAM_METRIC_VALUE_CLASS}>
                          {team.matchedMembers.length}
                        </div>
                      </div>
                      <div className={NODE_TEAM_METRIC_ITEM_CLASS}>
                        <div className={NODE_TEAM_METRIC_LABEL_CLASS}>Coordinators</div>
                        <div className={NODE_TEAM_METRIC_VALUE_CLASS}>
                          {team.matchedMembers.filter((member) => member.role === "coordinator").length}
                        </div>
                      </div>
                      <div className={NODE_TEAM_METRIC_ITEM_CLASS}>
                        <div className={NODE_TEAM_METRIC_LABEL_CLASS}>Workers</div>
                        <div className={NODE_TEAM_METRIC_VALUE_CLASS}>
                          {team.matchedMembers.filter((member) => member.role === "worker").length}
                        </div>
                      </div>
                    </div>
                  </div>
                  <div className="mt-3 border-t border-ui-border/60 pt-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                        Member Runtime Drill-down
                      </Text>
                      <Text size="xs" c="dimmed">
                        Jump straight into the thread or console without re-finding the member inside the
                        team shell.
                      </Text>
                    </div>
                    <div className="mt-2 grid gap-2 lg:grid-cols-2">
                      {team.matchedMembers.map((member) => {
                        const acpPath = buildTeamMemberAcpPath(team.teamId, member.memberId);
                        const consolePath = buildTeamMemberConsolePath(
                          team.teamId,
                          member.memberId
                        );
                        return (
                          <div key={member.memberId} className={NODE_MEMBER_DRILLDOWN_CLASS}>
                            <a
                              href={acpPath}
                              className="min-w-0 flex-1 truncate text-notion-text no-underline"
                              title={`Open thread for ${member.label}`}
                              onClick={(event) => handleInAppLinkClick(event, acpPath)}
                            >
                              {member.label}
                              {member.role ? ` · ${member.role}` : ""}
                            </a>
                            <div className="flex shrink-0 items-center gap-1">
                              <a
                                href={acpPath}
                                className={NODE_MEMBER_ACTION_CLASS}
                                title={`Open member thread for ${member.label}`}
                                onClick={(event) => handleInAppLinkClick(event, acpPath)}
                              >
                                Thread
                              </a>
                              <a
                                href={consolePath}
                                className={NODE_MEMBER_ACTION_CLASS}
                                title={`Open member console for ${member.label}`}
                                onClick={(event) => handleInAppLinkClick(event, consolePath)}
                              >
                                Console
                              </a>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <EmptyState
              title="No team attachments yet"
              body="No current team members resolve to this node from the global team catalog."
              className="mt-3 border border-dashed border-ui-border bg-white/80 px-3 py-4"
            />
          )}
        </div>
      </div>
    </div>
  );
}
