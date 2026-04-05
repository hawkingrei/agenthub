const RESERVED_AGENT_NODE_ID = "main";

function validateAgentNodeMutableFields(input: {
  nodeName: string;
  grpcTarget: string;
}): string | null {
  if (!input.nodeName.trim()) {
    return "Node name is required.";
  }
  if (!input.grpcTarget.trim()) {
    return "gRPC target is required.";
  }
  return null;
}

export function validateAgentNodeDraft(input: {
  nodeId: string;
  nodeName: string;
  grpcTarget: string;
}): string | null {
  const nodeId = input.nodeId.trim();
  if (!nodeId) {
    return "Node ID is required.";
  }
  if (nodeId === RESERVED_AGENT_NODE_ID) {
    return `Node ID '${RESERVED_AGENT_NODE_ID}' is reserved.`;
  }
  if (nodeId.length > 128) {
    return "Node ID must be at most 128 characters.";
  }
  if (![...nodeId].every((ch) => /[A-Za-z0-9._:-]/.test(ch))) {
    return "Node ID may only contain ASCII letters, numbers, '.', '_', '-', or ':'.";
  }
  return validateAgentNodeMutableFields(input);
}

export function validateAgentNodeUpdateDraft(input: {
  nodeName: string;
  grpcTarget: string;
}): string | null {
  return validateAgentNodeMutableFields(input);
}
