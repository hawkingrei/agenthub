import type { TeamSidebar } from "../team_sidebar";
import type {
  TeamWorkbenchContent,
  TeamWorkbenchContentProps,
} from "./team_workbench_content";
import type { TeamWorkspaceHeader } from "./team_workspace_header";
import type { TeamRunPanel } from "../team_run_panel";
import type { TeamPageModals } from "./team_page_modals";

export function buildTeamSidebarProps(
  props: React.ComponentProps<typeof TeamSidebar>
): React.ComponentProps<typeof TeamSidebar> {
  return props;
}

export function buildTeamWorkbenchContentProps(
  props: React.ComponentProps<typeof TeamWorkbenchContent>
): React.ComponentProps<typeof TeamWorkbenchContent> {
  return props;
}

export function buildTeamWorkbenchBodyProps(
  props: Pick<
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
  >
): Pick<
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
> {
  return props;
}

export function buildTeamWorkspaceHeaderProps(
  props: React.ComponentProps<typeof TeamWorkspaceHeader>
): React.ComponentProps<typeof TeamWorkspaceHeader> {
  return props;
}

export function buildTeamRunsPanelProps(
  props: React.ComponentProps<typeof TeamRunPanel>
): React.ComponentProps<typeof TeamRunPanel> {
  return props;
}

export function buildTeamPageModalsProps(
  props: React.ComponentProps<typeof TeamPageModals>
): React.ComponentProps<typeof TeamPageModals> {
  return props;
}
