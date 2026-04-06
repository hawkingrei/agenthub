import { AgentNodeSectionProps } from "./agent_node_section";
import { CreateAgentModalProps } from "./create_agent_modal";
import { PermissionModalProps } from "./permission_modal";

export function buildCreateAgentModalProps(
  props: CreateAgentModalProps
): CreateAgentModalProps {
  return props;
}

export function buildAgentNodeSectionProps(
  props: AgentNodeSectionProps | null
): AgentNodeSectionProps | null {
  return props;
}

export function buildPermissionModalProps(
  props: PermissionModalProps | null
): PermissionModalProps | null {
  return props;
}
