import React from "react";
import { Alert, Stack, Text } from "@mantine/core";
import {
  AgentNodeJoinBootstrapInfo,
  AgentNodeRecord,
  AgentRecord,
} from "../api";
import { isAgentActiveStatus } from "../agent_ws";
import {
  ActionButton,
  Badge,
  EmptyState,
  KeyValueItem,
  KeyValueList,
} from "../ui/primitives";
import { SECTION_HEADER_CLASS } from "../ui/tailwind_classes";

export const MACHINE_DETAIL_SECTION_CLASS =
  "rounded-xl border border-ui-border/80 bg-white/72 px-3 py-3";
export const MACHINE_DETAIL_AGENT_ROW_CLASS =
  "rounded-lg border border-ui-border/70 bg-white/82 px-3 py-2";
const MACHINE_DETAIL_METRIC_ITEM_CLASS =
  "rounded-xl border border-ui-border/70 bg-white/85 px-2.5 py-2 text-left shadow-[0_1px_2px_rgba(15,23,42,0.03)]";
const MACHINE_DETAIL_METRIC_LABEL_CLASS =
  "text-[10px] font-bold uppercase tracking-[0.08em] text-notion-text-muted/80";
const MACHINE_DETAIL_METRIC_VALUE_CLASS =
  "mt-1 text-[13px] font-semibold text-notion-text";
const MACHINE_DETAIL_SECTION_HEADER_CLASS =
  "flex flex-wrap items-start justify-between gap-3";

export function resolveAvailableNodes(nodes: AgentNodeRecord[]): AgentNodeRecord[] {
  return nodes.length > 0
    ? nodes
    : [
        {
          id: "main",
          name: "Main Node",
          grpc_target: null,
          tls_server_name: null,
          default_worktree_root: null,
          last_seen_at: null,
          is_main: true,
          created_at: 0,
          updated_at: 0,
        },
      ];
}

export function formatNodeTimestamp(timestamp: number | null | undefined): string {
  if (!timestamp || timestamp <= 0) {
    return "Not recorded";
  }
  return new Date(timestamp * 1000).toLocaleString();
}

export function resolveNodeRoleLabel(node: AgentNodeRecord): string {
  return node.is_main ? "local" : "remote";
}

export function describeSelectedNode(node: AgentNodeRecord | null): string {
  if (!node) {
    return "Select a node to inspect runtime metadata and attached agents.";
  }
  if (node.is_main) {
    return "This AgentHub instance is the local control plane and default execution target.";
  }
  return `Remote execution routes through encrypted gRPC${node.tls_server_name ? ` (${node.tls_server_name})` : ""}.`;
}

export function describeAgentAttachment(agent: AgentRecord): string {
  const worktreeModeLabel = formatWorktreeModeLabel(agent.worktree_mode);
  if (!worktreeModeLabel) {
    return "";
  }
  return `Worktree: ${worktreeModeLabel}`;
}

type AgentRuntimeLabel = {
  label: string;
  tone: "subtle" | "outline";
};

export function resolveAgentRuntimeLabels(agent: AgentRecord): AgentRuntimeLabel[] {
  const command = (agent.command ?? "").trim().toLowerCase();
  const labels = new Map<string, AgentRuntimeLabel>();
  if (command.includes("agenthub")) {
    labels.set("AgentHub Runtime", {
      label: "AgentHub Runtime",
      tone: "outline",
    });
  }
  if (command.includes("codex") || agent.code_mode) {
    labels.set("Codex CLI", {
      label: "Codex CLI",
      tone: "subtle",
    });
  }
  if (command.includes("gemini")) {
    labels.set("Gemini CLI", {
      label: "Gemini CLI",
      tone: "subtle",
    });
  }
  if (labels.size === 0) {
    labels.set("Custom Runtime", {
      label: "Custom Runtime",
      tone: "outline",
    });
  }
  return Array.from(labels.values());
}

function formatAgentStatusLabel(status: string): string {
  const normalized = status.trim().toLowerCase();
  switch (normalized) {
    case "running":
    case "working":
      return "Working";
    case "idle":
      return "Idle";
    case "stopped":
    case "exited":
      return "Stopped";
    default:
      return normalized
        ? normalized.replace(/[_-]+/g, " ").replace(/\b\w/g, (char) => char.toUpperCase())
        : "Unknown";
  }
}

function formatWorktreeModeLabel(
  worktreeMode: AgentRecord["worktree_mode"] | null | undefined
): string | null {
  switch (worktreeMode) {
    case "use_existing":
      return "Existing workdir";
    case "create_worktree":
      return "Fresh worktree";
    case "reuse_worktree":
      return "Shared worktree";
    default:
      return null;
  }
}

type NodeRuntimeSummary = {
  status: "connected" | "degraded" | "offline";
  label: string;
  tone: "subtle" | "outline";
  hint: string;
};

type NodeDetectedRuntime = {
  label: string;
  available: boolean;
};

const NODE_LAST_SEEN_RECENT_WINDOW_SECONDS = 10 * 60;

export function deriveNodeRuntimeSummary(
  node: AgentNodeRecord,
  agents: AgentRecord[]
): NodeRuntimeSummary {
  if (node.is_main) {
    return {
      status: "connected",
      label: "Connected",
      tone: "subtle",
      hint: "Local control plane node. This is the only node whose connectivity is directly implied by the current process.",
    };
  }
  if (node.last_seen_at && node.last_seen_at > 0) {
    const ageSeconds = Math.max(
      0,
      Math.floor(Date.now() / 1000) - node.last_seen_at
    );
    if (ageSeconds <= NODE_LAST_SEEN_RECENT_WINDOW_SECONDS) {
      return {
        status: "connected",
        label: "Connected",
        tone: "subtle",
        hint: `This node most recently bootstrapped ${formatNodeTimestamp(node.last_seen_at)}.`,
      };
    }
    return {
      status: "degraded",
      label: "Degraded",
      tone: "outline",
      hint: `The latest bootstrap credential issuance was recorded ${formatNodeTimestamp(node.last_seen_at)}, so the node identity is known but not recently refreshed.`,
    };
  }
  if (agents.some((agent) => isAgentActiveStatus(agent.status))) {
    return {
      status: "degraded",
      label: "Degraded",
      tone: "subtle",
      hint: "At least one attached agent is currently active. This is an indirect runtime signal, not a node heartbeat.",
    };
  }
  return {
    status: "offline",
    label: "Offline",
    tone: "outline",
    hint: "AgentHub currently has registry metadata for this node, but no direct heartbeat or last-seen signal.",
  };
}

export function deriveDetectedNodeRuntimes(
  node: AgentNodeRecord,
  agents: AgentRecord[]
): NodeDetectedRuntime[] {
  const observed = new Set<string>();
  if (node.is_main) {
    observed.add("AgentHub Control Plane");
  }
  for (const agent of agents) {
    const command = (agent.command ?? "").trim().toLowerCase();
    if (command.includes("codex") || agent.code_mode) {
      observed.add("Codex CLI");
    }
    if (command.includes("gemini")) {
      observed.add("Gemini CLI");
    }
    if (command.includes("agenthub")) {
      observed.add("AgentHub Runtime");
    }
  }
  const orderedLabels = [
    "AgentHub Control Plane",
    "AgentHub Runtime",
    "Codex CLI",
    "Gemini CLI",
  ] as const;
  const tags: NodeDetectedRuntime[] = [];
  for (const label of orderedLabels) {
    if (observed.has(label)) {
      tags.push({ label, available: true });
      continue;
    }
    if (label === "Codex CLI" || label === "Gemini CLI") {
      tags.push({
        label: `${label} (not detected)`,
        available: false,
      });
    }
  }
  return tags;
}

function escapeShellValue(value: string): string {
  return `'${value.replace(/'/g, "'\"'\"'")}'`;
}

async function copyTextToClipboard(text: string): Promise<void> {
  if (
    typeof navigator !== "undefined" &&
    navigator.clipboard &&
    typeof navigator.clipboard.writeText === "function"
  ) {
    await navigator.clipboard.writeText(text);
    return;
  }
  if (typeof document === "undefined") {
    throw new Error("clipboard unavailable");
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(textarea);
  if (!ok) {
    throw new Error("clipboard write failed");
  }
}

type NodeConnectCommandSpec = {
  command: string;
  substituted: string[];
  manual: string[];
  hasBootstrapToken: boolean;
  configItems: Array<{ label: string; value: string }>;
};

export function buildNodeConnectCommandSpec(args: {
  node: AgentNodeRecord;
  bootstrap: AgentNodeJoinBootstrapInfo | null;
}): NodeConnectCommandSpec | null {
  const { node, bootstrap } = args;
  if (node.is_main) {
    return null;
  }
  const token = bootstrap?.bootstrap_token?.trim();
  return {
    command: [
      `AGENTHUB_SERVER_ROLE=node`,
      `AGENTHUB_SERVER_NODE_ID=${escapeShellValue(node.id)}`,
      `AGENTHUB_INTERNAL_GRPC_ENABLED=true`,
      `AGENTHUB_INTERNAL_GRPC_LISTEN='0.0.0.0:50051'`,
      `AGENTHUB_INTERNAL_GRPC_BOOTSTRAP_TOKEN=${escapeShellValue(token || "<bootstrap-token-from-main-control-plane>")}`,
      `agenthub --config /etc/agenthub/config.toml`,
    ].join(" "),
    substituted: [
      `node_id=${node.id}`,
      ...(token ? ["bootstrap token"] : []),
    ],
    manual: [
      "config path",
      "listen addr if 50051 is not correct",
      ...(token ? [] : ["bootstrap token"]),
      "TLS/auth config parity with the main control plane",
    ],
    hasBootstrapToken: Boolean(token),
    configItems: [
      { label: "server.role", value: "node" },
      { label: "server.node_id", value: node.id },
      { label: "internal_grpc.enabled", value: "true" },
      { label: "internal_grpc.listen", value: "0.0.0.0:50051" },
      {
        label: "internal_grpc.bootstrap.token",
        value: token || "<bootstrap-token-from-main-control-plane>",
      },
      { label: "config file", value: "/etc/agenthub/config.toml" },
    ],
  };
}

type AgentNodeDetailCardProps = {
  node: AgentNodeRecord;
  agents: AgentRecord[];
  nodeJoinBootstrap: AgentNodeJoinBootstrapInfo | null;
  nodeJoinBootstrapLoading: boolean;
  nodeJoinBootstrapError: string | null;
  onOpenAgent?: (agentId: string) => void;
  onCreateAgent?: () => void;
  compact?: boolean;
};

export function AgentNodeDetailCard({
  node,
  agents,
  nodeJoinBootstrap,
  nodeJoinBootstrapLoading,
  nodeJoinBootstrapError,
  onOpenAgent,
  onCreateAgent,
  compact = false,
}: AgentNodeDetailCardProps) {
  const connectCommand = buildNodeConnectCommandSpec({ node, bootstrap: nodeJoinBootstrap });
  const runtimeSummary = deriveNodeRuntimeSummary(node, agents);
  const detectedRuntimes = React.useMemo(
    () => deriveDetectedNodeRuntimes(node, agents),
    [node, agents]
  );
  const infoItems = React.useMemo(
    () =>
      node.is_main
        ? [
            { label: "Node ID", value: node.id },
            { label: "Role", value: "Local control plane" },
            { label: "TLS server name", value: "Uses target host" },
            {
              label: "Registry evidence",
              value:
                "Local node identity is implied by the current AgentHub process rather than remote bootstrap metadata.",
            },
          ]
        : [
            { label: "Node ID", value: node.id },
            { label: "Role", value: "Remote execution node" },
            { label: "TLS server name", value: node.tls_server_name ?? "Uses target host" },
            { label: "Created", value: formatNodeTimestamp(node.created_at) },
            { label: "Updated", value: formatNodeTimestamp(node.updated_at) },
            { label: "Last seen", value: formatNodeTimestamp(node.last_seen_at) },
            {
              label: "Registry evidence",
              value: node.last_seen_at
                ? "The latest bootstrap credential issuance is persisted as a lightweight node last-seen signal."
                : "No bootstrap-based last-seen signal is persisted for this node yet.",
            },
          ],
    [node]
  );
  const [copied, setCopied] = React.useState(false);
  const [copyError, setCopyError] = React.useState<string | null>(null);
  const resetCopiedTimeoutRef = React.useRef<number | null>(null);
  const connectTone =
    runtimeSummary.status === "connected" && (node.is_main || connectCommand?.hasBootstrapToken)
      ? "border-ui-border/80 bg-white/72"
      : "border-amber-300 bg-amber-50/70";
  const showConnectFirst = runtimeSummary.status !== "connected";

  React.useEffect(() => {
    return () => {
      if (resetCopiedTimeoutRef.current !== null) {
        window.clearTimeout(resetCopiedTimeoutRef.current);
      }
    };
  }, []);

  const handleCopyConnectCommand = React.useCallback(async () => {
    if (!connectCommand) {
      return;
    }
    try {
      await copyTextToClipboard(connectCommand.command);
      setCopied(true);
      setCopyError(null);
      if (resetCopiedTimeoutRef.current !== null) {
        window.clearTimeout(resetCopiedTimeoutRef.current);
      }
      resetCopiedTimeoutRef.current = window.setTimeout(() => {
        setCopied(false);
        resetCopiedTimeoutRef.current = null;
      }, 1600);
    } catch (error) {
      setCopied(false);
      setCopyError(error instanceof Error ? error.message : "Copy failed");
    }
  }, [connectCommand]);

  return (
    <Stack gap="sm">
      <div className="rounded-xl border border-ui-border/80 bg-white/90 px-3 py-3">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <Text size={compact ? "md" : "lg"} fw={700}>
                {node.name}
              </Text>
              <Badge tone={node.is_main ? "subtle" : "outline"} className="uppercase">
                {resolveNodeRoleLabel(node)}
              </Badge>
              <Badge tone={runtimeSummary.tone}>{runtimeSummary.label}</Badge>
            </div>
            <Text size="sm" c="dimmed" mt={4}>
              {describeSelectedNode(node)}
            </Text>
            <Text size="xs" c="dimmed" mt={6}>
              {runtimeSummary.hint}
            </Text>
          </div>
          <Badge tone="outline">
            {agents.length} attached agent{agents.length === 1 ? "" : "s"}
          </Badge>
        </div>
        <div className="mt-3 grid gap-2 sm:grid-cols-3">
          <div className={MACHINE_DETAIL_METRIC_ITEM_CLASS}>
            <div className={MACHINE_DETAIL_METRIC_LABEL_CLASS}>Runtime signal</div>
            <div className={MACHINE_DETAIL_METRIC_VALUE_CLASS}>{runtimeSummary.label}</div>
          </div>
          <div className={MACHINE_DETAIL_METRIC_ITEM_CLASS}>
            <div className={MACHINE_DETAIL_METRIC_LABEL_CLASS}>Route target</div>
            <div className={`${MACHINE_DETAIL_METRIC_VALUE_CLASS} break-all`}>
              {node.is_main ? "local control plane" : (node.grpc_target ?? "encrypted gRPC")}
            </div>
          </div>
          <div className={MACHINE_DETAIL_METRIC_ITEM_CLASS}>
            <div className={MACHINE_DETAIL_METRIC_LABEL_CLASS}>Worktree root</div>
            <div className={`${MACHINE_DETAIL_METRIC_VALUE_CLASS} break-all`}>
              {node.default_worktree_root ?? "Explicit workdir required"}
            </div>
          </div>
        </div>
      </div>

      <div className="grid gap-3 xl:grid-cols-2">
        <div className={`${MACHINE_DETAIL_SECTION_CLASS} ${showConnectFirst ? "xl:order-2" : ""}`}>
          <div className={MACHINE_DETAIL_SECTION_HEADER_CLASS}>
            <div>
              <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                Info
              </Text>
              <Text size="xs" c="dimmed" mt={4}>
                Stable registry identity and lightweight runtime evidence for this node.
              </Text>
            </div>
          </div>
          <KeyValueList className="mt-3 grid gap-2">
            {infoItems.map((item) => (
              <KeyValueItem
                key={item.label}
                label={item.label}
                value={item.value}
                labelClassName="text-[10px] uppercase tracking-[0.08em] text-ui-text-muted"
                valueClassName="text-xs text-ui-text break-all"
              />
            ))}
          </KeyValueList>
          <div className="mt-3 rounded-lg border border-ui-border/80 bg-white/80 px-3 py-3">
            <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
              Detected Runtimes
            </Text>
            <Text size="xs" c="dimmed" mt={4}>
              Derived from current attached agents and known operator-facing runtime surfaces.
            </Text>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {detectedRuntimes.map((runtime) => (
                <Badge
                  key={runtime.label}
                  tone={runtime.available ? "subtle" : "outline"}
                >
                  {runtime.label}
                </Badge>
              ))}
            </div>
          </div>
        </div>

        <div
          className={`${MACHINE_DETAIL_SECTION_CLASS} ${connectTone} ${showConnectFirst ? "xl:order-1" : ""}`}
        >
          <div className={MACHINE_DETAIL_SECTION_HEADER_CLASS}>
            <div>
              <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                Connect Command
              </Text>
              <Text size="xs" c="dimmed" mt={4}>
                Bootstrap this node from the control plane, or inspect the canonical reconnect
                contract if you need to wire it into longer-lived infra.
              </Text>
            </div>
          </div>
          <div className="mt-3">
            {node.is_main ? (
              <Text size="sm" c="dimmed">
                The local control plane node does not need a remote bootstrap command.
              </Text>
            ) : nodeJoinBootstrapLoading ? (
              <Text size="sm" c="dimmed">
                Loading node bootstrap details...
              </Text>
            ) : nodeJoinBootstrapError ? (
              <Alert color="red" variant="light" title="Bootstrap unavailable">
                <Text size="sm">{nodeJoinBootstrapError}</Text>
              </Alert>
            ) : connectCommand ? (
              <Stack gap="xs">
                <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                  Connect Command
                </Text>
                <Text size="sm" c="dimmed">
                  Start the node process with the registered node id, then keep the process
                  running. The command below only substitutes values this page can derive
                  canonically.
                </Text>
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="flex flex-wrap gap-1.5">
                    {connectCommand.substituted.map((value) => (
                      <Badge key={value} tone="subtle">
                        {value}
                      </Badge>
                    ))}
                    {connectCommand.manual.map((value) => (
                      <Badge key={value} tone="outline">
                        needs: {value}
                      </Badge>
                    ))}
                  </div>
                  <ActionButton tone="secondary" size="sm" onClick={() => void handleCopyConnectCommand()}>
                    {copied ? "Copied" : "Copy"}
                  </ActionButton>
                </div>
                <pre className="overflow-x-auto rounded-lg border border-ui-border/80 bg-slate-950 px-3 py-3 text-[12px] leading-5 text-slate-50">
                  {connectCommand.command}
                </pre>
                {copyError ? (
                  <Text size="xs" c="red">
                    {copyError}
                  </Text>
                ) : null}
                {!connectCommand.hasBootstrapToken ? (
                  <Text size="xs" c="dimmed">
                    Bootstrap data is missing from the current API response, so the command keeps an
                    explicit token placeholder instead of pretending it is fully resolved.
                  </Text>
                ) : null}
                <div className="mt-2 rounded-lg border border-ui-border/80 bg-white/80 px-3 py-3">
                  <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
                    Connect Config
                  </Text>
                  <KeyValueList className="mt-3 grid gap-2">
                    {connectCommand.configItems.map((item) => (
                      <KeyValueItem
                        key={item.label}
                        label={item.label}
                        value={item.value}
                        labelClassName="text-[10px] uppercase tracking-[0.08em] text-ui-text-muted"
                        valueClassName="text-xs text-ui-text break-all"
                      />
                    ))}
                  </KeyValueList>
                </div>
              </Stack>
            ) : (
              <Text size="sm" c="dimmed">
                Bootstrap token details are not available yet. Load them from the root control
                plane before starting this node.
              </Text>
            )}
          </div>
        </div>
      </div>

      <div className={MACHINE_DETAIL_SECTION_CLASS}>
        <div className={MACHINE_DETAIL_SECTION_HEADER_CLASS}>
          <div className="min-w-0 flex-1">
            <Text size="xs" fw={700} c="dimmed" className={SECTION_HEADER_CLASS}>
              Agents on this node ({agents.length})
            </Text>
            <Text size="xs" c="dimmed" mt={4}>
              Route new agents here or open an attached agent for deeper runtime inspection.
            </Text>
          </div>
          {onCreateAgent ? (
            <ActionButton tone="secondary" size="sm" onClick={onCreateAgent}>
              Create Agent
            </ActionButton>
          ) : null}
        </div>
        {agents.length > 0 ? (
          <div className="mt-3 grid gap-2">
            {agents.map((agent) => (
              <div key={agent.id} className={MACHINE_DETAIL_AGENT_ROW_CLASS}>
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <Text size="sm" fw={600}>
                        {agent.name}
                      </Text>
                      <Badge tone="subtle" className="uppercase">
                        {formatAgentStatusLabel(agent.status)}
                      </Badge>
                      {resolveAgentRuntimeLabels(agent).map((runtime) => (
                        <Badge
                          key={`${agent.id}-${runtime.label}`}
                          tone={runtime.tone}
                        >
                          {runtime.label}
                        </Badge>
                      ))}
                    </div>
                    {describeAgentAttachment(agent) ? (
                      <Text size="xs" c="dimmed" mt={2}>
                        {describeAgentAttachment(agent)}
                      </Text>
                    ) : null}
                    <Text size="xs" c="dimmed" mt={6}>
                      {agent.workdir}
                    </Text>
                  </div>
                  {onOpenAgent ? (
                    <ActionButton
                      tone="secondary"
                      size="sm"
                      onClick={() => onOpenAgent(agent.id)}
                    >
                      Open
                    </ActionButton>
                  ) : null}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <EmptyState
            title="No agents on this node"
            body="Create or re-route an agent to make this node active."
            className="mt-3 border border-dashed border-ui-border bg-white/80 px-3 py-4"
          />
        )}
      </div>
    </Stack>
  );
}
