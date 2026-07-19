import {
  buildCanonicalTeamWorkspaceSubpath,
  buildTeamDetailPath as buildCanonicalTeamDetailPath,
  buildTeamWorkspacePath as buildLegacyTeamWorkspacePath,
  isTeamMemberRouteTab,
  resolveTeamRoute as resolveCanonicalTeamRoute,
  resolveWorkspaceLens,
  resolveTeamMemberRouteTab,
  resolveTeamWorkspacePathState,
  type TeamRouteState,
  type TeamMemberRouteTab,
  type WorkspaceLens,
} from "../../app_route_selection";
import {
  DEFAULT_TEAM_CHANNEL_ID,
  type TeamChannelId,
} from "./channel_metadata";
import type { TeamTab } from "./state";

export type { TeamMemberRouteTab, TeamRouteState, WorkspaceLens } from "../../app_route_selection";

export type TeamSidebarSubjectPane = "channels" | "tasks" | "agents";
export type TeamRouteSelection = {
  workspaceLens: WorkspaceLens;
  workspaceTab: TeamTab | null;
  channelId: TeamChannelId;
  threadRootMessageId: number | null;
  selectedMemberId: string;
  selectedTaskId: string;
};
export type TeamRouteLocation = {
  pathname: string;
  search: string;
};

function toAsciiLowercase(value: string): string {
  return value.replace(/[A-Z]/g, (char) => char.toLowerCase());
}

export function resolveTeamChannelId(search: string, pathname?: string): TeamChannelId {
  const params = new URLSearchParams(search);
  const pathChannel = pathname ? resolveTeamWorkspacePathState(pathname).channelId : null;
  const channel = (params.get("channel") ?? pathChannel ?? "").trim();
  return channel.length === 0 ? DEFAULT_TEAM_CHANNEL_ID : toAsciiLowercase(channel);
}

export function resolveTeamThreadRootMessageId(search: string, pathname?: string): number | null {
  const params = new URLSearchParams(search);
  const raw = (params.get("thread") ?? "").trim();
  if (raw) {
    const parsed = Number(raw);
    return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
  }
  return pathname ? resolveTeamWorkspacePathState(pathname).threadRootMessageId : null;
}

export function resolveTeamSelectedTaskId(search: string, pathname?: string): string {
  const params = new URLSearchParams(search);
  return (params.get("task") ?? resolveTeamWorkspacePathState(pathname ?? "").taskId ?? "").trim();
}

export function resolveTeamSelectedMemberId(search: string, pathname?: string): string {
  const params = new URLSearchParams(search);
  return (params.get("member") ?? resolveTeamWorkspacePathState(pathname ?? "").memberId ?? "").trim();
}

export function resolveTeamWorkspaceTab(search: string, pathname?: string): TeamTab | null {
  return resolveTeamMemberRouteTab(search) ?? resolveTeamWorkspacePathState(pathname ?? "").tab;
}

export function resolveTeamTabForWorkspaceLens(lens: WorkspaceLens): TeamTab | null {
  switch (lens) {
    case "channels":
      return "conversation";
    case "tasks":
      return "tasks";
    case "members":
      return "overview";
    case "search":
      return null;
    default:
      return "conversation";
  }
}

export function resolveWorkspaceLensForTeamTab(tab: TeamTab): WorkspaceLens {
  switch (tab) {
    case "tasks":
      return "tasks";
    case "overview":
      return "members";
    default:
      return "channels";
  }
}

export function resolveActiveTeamWorkspaceLens({
  routeWorkspaceLens,
  tab,
}: {
  routeWorkspaceLens: WorkspaceLens | null;
  tab: TeamTab;
}): WorkspaceLens {
  const tabWorkspaceLens = resolveWorkspaceLensForTeamTab(tab);
  return routeWorkspaceLens === "search" ? tabWorkspaceLens : routeWorkspaceLens ?? tabWorkspaceLens;
}

export function normalizeTeamWorkspaceLensForHeader(
  activeWorkspaceLens: WorkspaceLens
): WorkspaceLens {
  return activeWorkspaceLens === "search" ? "channels" : activeWorkspaceLens;
}

export function resolveTeamSidebarSubjectPane({
  tab,
  activeWorkspaceLens,
}: {
  tab: TeamTab;
  activeWorkspaceLens?: WorkspaceLens | null;
}): TeamSidebarSubjectPane {
  if (activeWorkspaceLens === "tasks") {
    return "tasks";
  }
  if (activeWorkspaceLens === "members") {
    return "agents";
  }
  if (tab === "tasks") {
    return "tasks";
  }
  if (tab === "agent_acp" || tab === "member_console" || tab === "mailbox") {
    return "agents";
  }
  return "channels";
}

export function buildTeamDetailPath(teamId: string): string {
  return buildCanonicalTeamDetailPath(teamId);
}

export function buildTeamSelectorPath(): string {
  return buildCanonicalTeamDetailPath(null);
}

export function resolveTeamRoute(pathname: string): TeamRouteState | null {
  return resolveCanonicalTeamRoute(pathname);
}

export function resolveTeamWorkspaceLens(pathname: string, search: string): WorkspaceLens {
  return resolveWorkspaceLens(pathname, search);
}

export function resolveTeamRouteSelection(pathname: string, search: string): TeamRouteSelection {
  return {
    workspaceLens: resolveTeamWorkspaceLens(pathname, search),
    workspaceTab: resolveTeamWorkspaceTab(search, pathname),
    channelId: resolveTeamChannelId(search, pathname),
    threadRootMessageId: resolveTeamThreadRootMessageId(search, pathname),
    selectedMemberId: resolveTeamSelectedMemberId(search, pathname),
    selectedTaskId: resolveTeamSelectedTaskId(search, pathname),
  };
}

export function splitTeamRoutePath(path: string): TeamRouteLocation {
  const url = new URL(path, "http://agenthub.local");
  return {
    pathname: url.pathname,
    search: url.search,
  };
}

export function buildTeamWorkspacePath(
  teamId: string,
  lens?: WorkspaceLens | null,
  channelId?: TeamChannelId | null,
  threadRootMessageId?: number | null,
  memberId?: string | null,
  tab?: TeamTab | null,
  taskId?: string | null
): string {
  const normalizedTab: TeamMemberRouteTab | null = isTeamMemberRouteTab(tab) ? tab : null;
  return buildLegacyTeamWorkspacePath(
    teamId,
    lens,
    channelId,
    threadRootMessageId,
    memberId,
    normalizedTab,
    taskId
  );
}

export function buildTeamChannelProfilePath(
  teamId: string,
  channelId: TeamChannelId | null,
  memberId: string,
  taskId?: string | null
): string {
  return buildCanonicalTeamSubpath(teamId, "channels", channelId, null, memberId, null, taskId);
}

export function buildTeamChannelProfileClosePath(
  teamId: string,
  channelId: TeamChannelId | null
): string {
  return buildTeamChannelPath(teamId, channelId);
}

export function buildTeamSearchCompatibilityPath(teamId: string): string {
  return `${buildTeamChannelPath(teamId)}?lens=search`;
}

export function buildTeamTabCompatibilityPath(pathname: string, tab: TeamTab): string {
  const params = new URLSearchParams();
  if (isTeamMemberRouteTab(tab)) {
    params.set("tab", tab === "agent_acp" ? "thread" : tab);
  } else {
    params.set("tab", tab);
  }
  return `${pathname}?${params.toString()}`;
}

export function buildTeamChannelPath(
  teamId: string,
  channelId?: TeamChannelId | null
): string {
  return buildCanonicalTeamSubpath(teamId, "channels", channelId);
}

export function buildTeamChannelThreadPath(
  teamId: string,
  channelId: TeamChannelId | null,
  threadRootMessageId: number,
  taskId?: string | null
): string {
  return buildCanonicalTeamSubpath(
    teamId,
    "channels",
    channelId,
    threadRootMessageId,
    null,
    null,
    taskId
  );
}

export function buildTeamChannelTaskPath(
  teamId: string,
  channelId: TeamChannelId | null,
  taskId: string
): string {
  return buildCanonicalTeamSubpath(teamId, "channels", channelId, null, null, null, taskId);
}

export function buildTeamTaskPath(teamId: string, taskId?: string | null): string {
  return buildCanonicalTeamSubpath(teamId, "tasks", null, null, null, null, taskId);
}

export function buildTeamMemberWorkspacePath(
  teamId: string,
  memberId: string,
  tab: TeamTab
): string {
  return buildCanonicalTeamSubpath(teamId, "members", null, null, memberId, tab);
}

export function buildTeamLensNavigationPath(
  teamId: string,
  lens: WorkspaceLens,
  channelId?: TeamChannelId | null,
  taskId?: string | null
): string {
  if (lens === "channels") {
    return taskId
      ? buildTeamChannelTaskPath(teamId, channelId ?? DEFAULT_TEAM_CHANNEL_ID, taskId)
      : buildTeamChannelPath(teamId, channelId ?? DEFAULT_TEAM_CHANNEL_ID);
  }
  if (lens === "tasks") {
    return buildTeamTaskPath(teamId, taskId);
  }
  return buildCanonicalTeamSubpath(
    teamId,
    lens,
    null,
    null,
    null,
    null,
    null
  );
}

export function buildTeamWorkspaceLensPath(
  teamId: string,
  lens: WorkspaceLens,
  channelId?: TeamChannelId | null
): string {
  if (lens === "channels" || lens === "search") {
    return buildTeamChannelPath(teamId, channelId);
  }
  return buildTeamLensNavigationPath(teamId, lens);
}

export function buildCanonicalTeamSubpath(
  teamId: string,
  lens?: WorkspaceLens | null,
  channelId?: TeamChannelId | null,
  threadRootMessageId?: number | null,
  memberId?: string | null,
  tab?: TeamTab | null,
  taskId?: string | null
): string {
  const normalizedTab: TeamMemberRouteTab | null = isTeamMemberRouteTab(tab) ? tab : null;
  return buildCanonicalTeamWorkspaceSubpath(
    teamId,
    lens,
    channelId,
    threadRootMessageId,
    memberId,
    normalizedTab,
    taskId
  );
}
