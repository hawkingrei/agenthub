import React from "react";
import { EmptyState, SurfaceCard } from "../ui/primitives";

export const SHARED_WORKSPACE_SEARCH_PLACEHOLDER_TITLE =
  "Shared search is still being wired in";
export const SHARED_WORKSPACE_SEARCH_PLACEHOLDER_BODY =
  "Use Channels, Tasks, or Members while the unified workspace search view is still a shell-level placeholder.";
export const SHARED_WORKSPACE_SEARCH_LENS_HINT =
  "Shared search is still being wired in across the unified workspace shell.";
export const WORKSPACE_MACHINES_UNAVAILABLE_TITLE = "Machines unavailable";
export const WORKSPACE_MACHINES_UNAVAILABLE_BODY =
  "You do not have permission to manage machines. Select another workspace view to continue.";

type WorkspaceLensPlaceholderProps = {
  lensLabel: string;
  title: string;
  body: string;
  className?: string;
};

export const WorkspaceLensPlaceholder = React.memo(function WorkspaceLensPlaceholder({
  lensLabel,
  title,
  body,
  className = "",
}: WorkspaceLensPlaceholderProps) {
  return (
    <SurfaceCard className={className} data-workspace-lens-placeholder={lensLabel.toLowerCase()}>
      <div className="text-[10px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
        {lensLabel}
      </div>
      <EmptyState className="mt-2 border-0 bg-transparent px-0 py-0" title={title} body={body} />
    </SurfaceCard>
  );
});

type WorkspaceSearchLensPlaceholderProps = {
  className?: string;
};

export function WorkspaceSearchLensPlaceholder({
  className = "",
}: WorkspaceSearchLensPlaceholderProps) {
  return (
    <WorkspaceLensPlaceholder
      lensLabel="Search"
      title={SHARED_WORKSPACE_SEARCH_PLACEHOLDER_TITLE}
      body={SHARED_WORKSPACE_SEARCH_PLACEHOLDER_BODY}
      className={className}
    />
  );
}

type WorkspaceMachinesUnavailablePlaceholderProps = {
  className?: string;
};

export function WorkspaceMachinesUnavailablePlaceholder({
  className = "",
}: WorkspaceMachinesUnavailablePlaceholderProps) {
  return (
    <WorkspaceLensPlaceholder
      lensLabel="Machines"
      title={WORKSPACE_MACHINES_UNAVAILABLE_TITLE}
      body={WORKSPACE_MACHINES_UNAVAILABLE_BODY}
      className={className}
    />
  );
}
