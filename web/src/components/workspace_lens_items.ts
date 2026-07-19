import type { WorkspaceLens } from "../app_route_selection";
import type { WorkspaceShellLensItem } from "./workspace_shell_header";

type BuildStandardWorkspaceLensItemsOptions = {
  includeNodes?: boolean;
  onPrefetch?: (lens: WorkspaceLens) => void;
};

export function buildStandardWorkspaceLensItems(
  activeLens: WorkspaceLens,
  options: BuildStandardWorkspaceLensItemsOptions = {}
): WorkspaceShellLensItem[] {
  const { includeNodes = false, onPrefetch } = options;
  const items: WorkspaceShellLensItem[] = [
    {
      value: "teams",
      label: "Teams",
      active: activeLens === "teams",
      onPrefetch: onPrefetch ? () => onPrefetch("teams") : undefined,
    },
    {
      value: "channels",
      label: "Channels",
      active: activeLens === "channels",
      onPrefetch: onPrefetch ? () => onPrefetch("channels") : undefined,
    },
    {
      value: "tasks",
      label: "Tasks",
      active: activeLens === "tasks",
      onPrefetch: onPrefetch ? () => onPrefetch("tasks") : undefined,
    },
    {
      value: "members",
      label: "Members",
      active: activeLens === "members",
      onPrefetch: onPrefetch ? () => onPrefetch("members") : undefined,
    },
  ];
  if (includeNodes) {
    items.push({
      value: "nodes",
      label: "Machines",
      active: activeLens === "nodes",
    });
  }
  return items;
}
