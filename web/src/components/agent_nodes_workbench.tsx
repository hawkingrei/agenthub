import React from "react";
import { Button, Stack, Text, TextInput } from "@mantine/core";
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

const NODE_ROSTER_ITEM_CLASS =
  "w-full rounded-xl border-2 border-transparent bg-white/70 px-2.5 py-2.5 shadow-none hover:border-black hover:bg-white";
const NODE_ROSTER_ITEM_ACTIVE_CLASS = "border-black bg-white";

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
  nodeId: string
): NodeTeamUsageSummary[] {
  const agentsById = new Map(agents.map((agent) => [agent.id, agent]));
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
        return status === "running";
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

type AgentNodesWorkbenchProps = {
  nodes: AgentNodeRecord[];
  agents: AgentRecord[];
  teams?: TeamDefinitionRecord[];
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
      summaries.set(node.id, deriveNodeTeamUsageSummaries(teams, agents, node.id));
    }
    return summaries;
  }, [agents, availableNodes, teams]);
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
  const selectedNodeLeaderCount = selectedNodeTeams.reduce(
    (sum, team) =>
      sum + team.matchedMembers.filter((member) => member.role === "leader").length,
    0
  );
  const selectedNodeWorkerCount = selectedNodeTeams.reduce(
    (sum, team) =>
      sum + team.matchedMembers.filter((member) => member.role === "worker").length,
    0
  );
  const [editDrafts, setEditDrafts] = React.useState<
    Record<
      string,
      {
        name: string;
        grpcTarget: string;
        tlsServerName: string;
        defaultWorktreeRoot: string;
      }
    >
  >({});

  React.useEffect(() => {
    setEditDrafts((prev) => {
      const next: typeof prev = {};
      for (const node of availableNodes) {
        if (node.is_main) {
          continue;
        }
        next[node.id] = prev[node.id] ?? {
          name: node.name,
          grpcTarget: node.grpc_target ?? "",
          tlsServerName: node.tls_server_name ?? "",
          defaultWorktreeRoot: node.default_worktree_root ?? "",
        };
      }
      return next;
    });
  }, [availableNodes]);

  if (!selectedNode) {
    return (
      <div className="flex h-full min-h-0 flex-col overflow-auto bg-white px-4 py-4 sm:px-6">
        <EmptyState
          title="No nodes registered yet"
          body="Register a remote node or use the local control plane node to start routing agents."
          className="border border-dashed border-ui-border bg-white/80 px-4 py-8"
        />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col overflow-auto bg-white px-4 py-4 sm:px-6">
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

        <div className="grid gap-4 xl:grid-cols-[260px_minmax(0,1fr)]">
          <div className="rounded-2xl border border-ui-border bg-ui-surface-soft/70 p-3 shadow-sm">
            <Stack gap="xs">
              <Text size="xs" fw={700} c="dimmed" className="uppercase tracking-[0.08em]">
                Nodes
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

          <AgentNodeDetailCard
            node={selectedNode}
            agents={selectedNodeAgents}
            nodeJoinBootstrap={nodeJoinBootstrap}
            nodeJoinBootstrapLoading={nodeJoinBootstrapLoading}
            nodeJoinBootstrapError={nodeJoinBootstrapError}
            onOpenAgent={onOpenAgent}
            onCreateAgent={onCreateAgent}
          />
          <div className="rounded-xl border border-ui-border/80 bg-white/72 px-4 py-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <Text size="xs" fw={700} c="dimmed" className="uppercase tracking-[0.08em]">
                  Teams Using This Node
                </Text>
                <Text size="sm" c="dimmed" mt={6}>
                  Because nodes are global resources, this section shows which teams currently land
                  members on the selected node rather than treating node usage as team-local state.
                </Text>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone="outline">
                  {selectedNodeTeams.length} team{selectedNodeTeams.length === 1 ? "" : "s"}
                </Badge>
                <Badge tone="outline">
                  {selectedNodeTeamMemberCount} members
                </Badge>
                <Badge tone="subtle">
                  {selectedNodeActiveTeamAgentCount} active
                </Badge>
                <Badge tone="outline">{selectedNodeLeaderCount} leaders</Badge>
                <Badge tone="outline">{selectedNodeWorkerCount} workers</Badge>
              </div>
            </div>
            {selectedNodeTeams.length > 0 ? (
              <div className="mt-3 grid gap-2">
                {selectedNodeTeams.map((team) => (
                  <div
                    key={team.teamId}
                    className="rounded-lg border border-ui-border/70 bg-white/85 px-3 py-3"
                  >
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <Text size="sm" fw={600}>
                            {team.teamName}
                          </Text>
                          <Badge tone="outline">{team.matchedMembers.length} members</Badge>
                          <Badge tone="subtle">{team.activeAgentCount} active</Badge>
                          <Badge tone="outline">
                            {team.matchedMembers.filter((member) => member.role === "leader").length} leaders
                          </Badge>
                          <Badge tone="outline">
                            {team.matchedMembers.filter((member) => member.role === "worker").length} workers
                          </Badge>
                        </div>
                        <Text size="xs" c="dimmed" mt={4}>
                          team_id={team.teamId}
                        </Text>
                        <div className="mt-3 flex flex-wrap gap-1.5">
                          {team.matchedMembers.map((member) => (
                            <Badge key={member.memberId} tone="outline">
                              {member.label}
                              {member.role ? ` · ${member.role}` : ""}
                            </Badge>
                          ))}
                        </div>
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
          {selectedNode.is_main ? (
            <div className="rounded-xl border border-ui-border/80 bg-white/72 px-4 py-4">
              <Text size="xs" fw={700} c="dimmed" className="uppercase tracking-[0.08em]">
                Danger Zone
              </Text>
              <Text size="sm" c="dimmed" mt={8}>
                The local control-plane node cannot be deleted or re-pointed from this surface.
              </Text>
            </div>
          ) : (
            <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_280px]">
              <div className="rounded-xl border border-ui-border/80 bg-white/72 px-4 py-4">
                <Stack gap="sm">
                  <div>
                    <Text size="xs" fw={700} c="dimmed" className="uppercase tracking-[0.08em]">
                      Settings
                    </Text>
                    <Text size="sm" c="dimmed" mt={6}>
                      Update routing and default worktree behavior for this node from the canonical
                      detail page.
                    </Text>
                  </div>
                  {(() => {
                    const draft = editDrafts[selectedNode.id] ?? {
                      name: selectedNode.name,
                      grpcTarget: selectedNode.grpc_target ?? "",
                      tlsServerName: selectedNode.tls_server_name ?? "",
                      defaultWorktreeRoot: selectedNode.default_worktree_root ?? "",
                    };
                    const updateError = validateAgentNodeUpdateDraft({
                      nodeName: draft.name,
                      grpcTarget: draft.grpcTarget,
                    });
                    return (
                      <>
                        <div className="grid gap-3 lg:grid-cols-2">
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
                        </div>
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
                              Boolean(updatingNodeIds[selectedNode.id]) || updateError !== null
                            }
                            onClick={() =>
                              onUpdateNode?.(selectedNode.id, {
                                name: draft.name.trim(),
                                grpc_target: draft.grpcTarget.trim(),
                                tls_server_name: draft.tlsServerName.trim() || null,
                                default_worktree_root: draft.defaultWorktreeRoot.trim() || null,
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
                    <Text size="xs" fw={700} c="red" className="uppercase tracking-[0.08em]">
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
          )}
        </div>
      </div>
    </div>
  );
}
