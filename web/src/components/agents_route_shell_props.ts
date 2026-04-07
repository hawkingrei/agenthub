import type { AgentRecord } from "../api";
import type { AgentsPanelProps } from "./agents_panel";
import type { AgentsWorkbenchProps } from "./agents_workbench_types";
import type { OutputHeaderProps } from "./output_header";

export function buildAgentsPanelProps(
  props: AgentsPanelProps
): AgentsPanelProps {
  return props;
}

export function buildOutputHeaderProps(
  props: OutputHeaderProps
): OutputHeaderProps {
  return props;
}

export type BuildAgentsWorkbenchPropsArgs = Omit<
  AgentsWorkbenchProps,
  "activeAgent"
> & {
  activeAgent: string | null;
  activeAgentRecord: AgentRecord | null;
};

export function buildAgentsWorkbenchProps({
  activeAgent,
  ...rest
}: BuildAgentsWorkbenchPropsArgs): AgentsWorkbenchProps | null {
  if (!activeAgent) {
    return null;
  }
  return {
    activeAgent,
    ...rest,
  };
}
