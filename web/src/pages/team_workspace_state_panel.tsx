import { ActionButton } from "../ui/primitives";
import { WorkspaceLensPlaceholder } from "../components/workspace_lens_placeholder";
import { WorkspacePanelLoadingFallback } from "../components/workspace_panel_loading_fallback";

export function TeamLoadingPanel() {
  return (
    <div className="mx-auto flex w-full max-w-[680px] flex-col px-6 py-10" data-team-loading-shell="true">
      <WorkspacePanelLoadingFallback
        title="Loading team workspace..."
        body="AgentHub is loading the workspace context and team metadata."
        className="border-notion-border/70 bg-white/92"
      />
    </div>
  );
}

type TeamUnavailablePanelProps = {
  onBackToSelector: () => void;
};

export function TeamUnavailablePanel({
  onBackToSelector,
}: TeamUnavailablePanelProps) {
  return (
    <div className="mx-auto flex w-full max-w-[680px] flex-col px-6 py-10">
      <WorkspaceLensPlaceholder
        lensLabel="Teams"
        title="This team is unavailable"
        body="The requested team could not be loaded. Return to the team list and choose another one."
      />
      <div className="mt-5">
        <ActionButton
          size="sm"
          tone="ghost"
          className="justify-start rounded-md px-0 text-[12px] font-medium text-notion-text-muted hover:text-notion-text"
          onClick={onBackToSelector}
        >
          Back to teams
        </ActionButton>
      </div>
    </div>
  );
}
