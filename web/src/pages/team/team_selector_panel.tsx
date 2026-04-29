import React from "react";
import { TextInput } from "@mantine/core";
import {
  ActionButton,
  IconButton,
  SelectableListItem,
  ToolbarRow,
} from "../../ui/primitives";
import { WorkspacePanelLoadingFallback } from "../../components/workspace_panel_loading_fallback";

export type TeamSelectorItem = {
  id: string;
  name: string;
  description: string;
  summary: string;
  runtimeLabel: string;
};

type TeamSelectorPanelProps = {
  busy: string | null;
  filter: string;
  loading: boolean;
  hasTeams: boolean;
  items: readonly TeamSelectorItem[];
  bodyTextClassName: string;
  accentButtonClassName: string;
  onFilterChange: (value: string) => void;
  onRefreshTeams: () => void;
  onCreateTeam: () => void;
  onSelectTeam: (teamId: string) => void;
};

const TEAM_SELECTOR_LIST_CLASS =
  "flex flex-col gap-0.5 [content-visibility:auto] [contain-intrinsic-size:1px_320px]";

const TeamSelectorEntry = React.memo(function TeamSelectorEntry({
  team,
  onSelectTeam,
}: {
  team: TeamSelectorItem;
  onSelectTeam: (teamId: string) => void;
}) {
  return (
    <SelectableListItem
      type="button"
      layout="row"
      className="w-full min-w-0 items-start justify-between gap-3 text-left"
      data-team-selector-entry="true"
      data-team-id={team.id}
      data-team-name={team.name}
      onClick={() => onSelectTeam(team.id)}
    >
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-medium text-notion-text">
          {team.name}
        </div>
        <div className="mt-0.5 line-clamp-1 text-[11px] leading-5 text-notion-text-muted">
          {team.description}
        </div>
        <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[10px] leading-4 text-notion-text-muted">
          <span className="truncate">{team.summary}</span>
          <span className="inline-flex h-1 w-1 shrink-0 rounded-full bg-notion-text-muted/35" />
          <span className="shrink-0 capitalize">{team.runtimeLabel}</span>
        </div>
      </div>
    </SelectableListItem>
  );
});

export const TeamSelectorPanel = React.memo(function TeamSelectorPanel({
  busy,
  filter,
  loading,
  hasTeams,
  items,
  bodyTextClassName,
  accentButtonClassName,
  onFilterChange,
  onRefreshTeams,
  onCreateTeam,
  onSelectTeam,
}: TeamSelectorPanelProps) {
  return (
    <div className="flex min-h-0 flex-1 justify-center">
      <section className="flex min-h-0 w-full max-w-[680px] flex-col">
        <ToolbarRow className="mb-1 mt-3 px-2">
          <div className="min-w-0 flex-1">
            <h2 className="text-[14px] font-medium tracking-tight text-notion-text-muted">Teams</h2>
          </div>
          <ActionButton
            type="button"
            tone="ghost"
            size="sm"
            className={`h-7 rounded-md px-2 text-[11px] text-notion-text-muted hover:bg-[rgba(55,53,47,0.05)] hover:text-notion-text ${accentButtonClassName}`}
            onClick={onCreateTeam}
          >
            New Team
          </ActionButton>
        </ToolbarRow>

        {!loading && hasTeams && (
          <ToolbarRow className="mt-1 gap-2 px-2">
            <TextInput
              className="flex-1"
              radius="md"
              placeholder="Search teams"
              aria-label="Filter teams"
              value={filter}
              onChange={(event) => onFilterChange(event.target.value)}
              variant="unstyled"
              classNames={{
                input:
                  "h-8 rounded-md border border-notion-border/70 bg-white/72 px-3 text-[12px] text-notion-text shadow-none placeholder:text-notion-text-muted focus:border-notion-border-subtle focus:bg-white",
              }}
            />
            <IconButton
              size="md"
              className="h-8 w-8 rounded-md bg-transparent text-notion-text-muted hover:bg-[rgba(55,53,47,0.05)] hover:text-notion-text"
              onClick={onRefreshTeams}
              disabled={busy === "refresh-teams"}
              aria-label="Refresh teams"
              title="Refresh teams"
            >
              <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            </IconButton>
          </ToolbarRow>
        )}

        <div className="mt-2 flex-1 overflow-y-auto">
          <div className={TEAM_SELECTOR_LIST_CLASS}>
            {loading && (
              <WorkspacePanelLoadingFallback
                title="Loading teams..."
                body="AgentHub is loading the available team workspaces."
                className="mx-2"
              />
            )}
            {!loading && !hasTeams && (
              <p className={`${bodyTextClassName} px-2`}>
                No teams yet. Create one to begin.
              </p>
            )}
            {!loading && hasTeams && items.length === 0 && (
              <p className={`${bodyTextClassName} px-2`}>No teams match the current filter.</p>
            )}
            {items.map((team) => (
              <TeamSelectorEntry key={team.id} team={team} onSelectTeam={onSelectTeam} />
            ))}
          </div>
        </div>
      </section>
    </div>
  );
});
