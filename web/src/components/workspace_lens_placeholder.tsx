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
const WORKSPACE_CROSS_ENTITY_CHANNELS_TITLE =
  "Workspace channels aggregate across teams";
const WORKSPACE_CROSS_ENTITY_CHANNELS_BODY =
  "Open a team to browse its channels and threads. A shared cross-team channel index will land in a future workspace shell phase.";
const WORKSPACE_CROSS_ENTITY_TASKS_TITLE =
  "Workspace tasks aggregate across teams";
const WORKSPACE_CROSS_ENTITY_TASKS_BODY =
  "Open a team to browse its Kanban board and active tasks. A shared cross-team task view will land in a future workspace shell phase.";
const WORKSPACE_CROSS_ENTITY_MEMBERS_TITLE =
  "Workspace members aggregate across teams";
const WORKSPACE_CROSS_ENTITY_MEMBERS_BODY =
  "Open a team to browse its member roster. A shared cross-team member directory will land in a future workspace shell phase.";

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
    <SurfaceCard className={className} data-workspace-lens-placeholder="search">
      <div className="flex flex-col gap-3">
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
            Search
          </div>
          <h3 className="mt-1 text-[16px] font-semibold tracking-tight text-notion-text">
            Search workspace
          </h3>
          <p className="mt-1 text-[13px] leading-5 text-notion-text-muted">
            Find Team messages, tasks, channels, and agent context.
          </p>
        </div>
        <label className="relative block">
          <span className="sr-only">Search workspace</span>
          <i
            className="bi bi-search pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[13px] text-notion-text-muted"
            aria-hidden="true"
          />
          <input
            className="h-10 w-full rounded-lg border border-notion-border bg-white pl-8 pr-3 text-[13px] text-notion-text outline-none transition placeholder:text-notion-text-muted/70 focus:border-notion-accent focus:ring-2 focus:ring-notion-accent/15"
            type="search"
            placeholder="Search messages, tasks, channels, or agents"
            aria-label="Search workspace"
          />
        </label>
      </div>
    </SurfaceCard>
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

export function WorkspaceChannelsLensPlaceholder({
  className = "",
}: {
  className?: string;
}) {
  return (
    <WorkspaceLensPlaceholder
      lensLabel="Channels"
      title={WORKSPACE_CROSS_ENTITY_CHANNELS_TITLE}
      body={WORKSPACE_CROSS_ENTITY_CHANNELS_BODY}
      className={className}
    />
  );
}

export function WorkspaceTasksLensPlaceholder({
  className = "",
}: {
  className?: string;
}) {
  return (
    <WorkspaceLensPlaceholder
      lensLabel="Tasks"
      title={WORKSPACE_CROSS_ENTITY_TASKS_TITLE}
      body={WORKSPACE_CROSS_ENTITY_TASKS_BODY}
      className={className}
    />
  );
}

export function WorkspaceMembersLensPlaceholder({
  className = "",
}: {
  className?: string;
}) {
  return (
    <WorkspaceLensPlaceholder
      lensLabel="Members"
      title={WORKSPACE_CROSS_ENTITY_MEMBERS_TITLE}
      body={WORKSPACE_CROSS_ENTITY_MEMBERS_BODY}
      className={className}
    />
  );
}
