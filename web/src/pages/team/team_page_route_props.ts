import type { ComponentProps } from "react";
import type { TeamSidebar } from "../team_sidebar";
import type { TeamRunPanel } from "../team_run_panel";
import type { TeamPageModals } from "./team_page_modals";
import type {
  TeamWorkbenchContentProps,
} from "./team_workbench_content";
import type { TeamWorkbenchContent } from "./team_workbench_content";
import type { TeamWorkspaceHeader } from "./team_workspace_header";

export function buildTeamSidebarProps(
  props: ComponentProps<typeof TeamSidebar>
): ComponentProps<typeof TeamSidebar> {
  return props;
}

export function buildTeamWorkbenchContentProps(
  props: ComponentProps<typeof TeamWorkbenchContent>
): ComponentProps<typeof TeamWorkbenchContent> {
  return props;
}

export type TeamWorkbenchBodyProps = Pick<
  TeamWorkbenchContentProps,
  | "conversationPanel"
  | "threadPane"
  | "tasksPanel"
  | "agentAcpPanel"
  | "overviewPanelProps"
  | "eventsPanelProps"
  | "stepsPanelProps"
  | "mailboxHasActiveRun"
  | "mailboxEmptyTitle"
  | "mailboxEmptyBody"
  | "onGoToRuns"
  | "mailboxPanelProps"
  | "memberConsolePanelProps"
  | "debugPanel"
>;

export function buildTeamWorkbenchBodyProps(
  props: TeamWorkbenchBodyProps
): TeamWorkbenchBodyProps {
  return props;
}

export function buildTeamWorkspaceHeaderProps(
  props: ComponentProps<typeof TeamWorkspaceHeader>
): ComponentProps<typeof TeamWorkspaceHeader> {
  return props;
}

export function buildTeamRunsPanelProps(
  props: ComponentProps<typeof TeamRunPanel>
): ComponentProps<typeof TeamRunPanel> {
  return props;
}

export function buildTeamPageModalsProps(
  props: ComponentProps<typeof TeamPageModals>
): ComponentProps<typeof TeamPageModals> {
  return props;
}
