import React from "react";
import { TextInput } from "@mantine/core";
import {
  ActionButton,
  IconButton,
  SelectableListItem,
  ToolbarRow,
} from "../../ui/primitives";

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
  hasTeams: boolean;
  items: readonly TeamSelectorItem[];
  bodyTextClassName: string;
  accentButtonClassName: string;
  onFilterChange: (value: string) => void;
  onRefreshTeams: () => void;
  onCreateTeam: () => void;
  onSelectTeam: (teamId: string) => void;
};

export const TeamSelectorPanel = React.memo(function TeamSelectorPanel({
  busy,
  filter,
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
        <ToolbarRow className="items-start px-2">
          <div className="min-w-0 flex-1">
            <h2 className="text-[17px] font-semibold tracking-tight text-black">Teams</h2>
          </div>
          <ActionButton
            type="button"
            tone="secondary"
            size="sm"
            className={accentButtonClassName}
            onClick={onCreateTeam}
          >
            New Team
          </ActionButton>
        </ToolbarRow>

        {hasTeams && (
          <ToolbarRow className="mt-2 justify-start gap-2 px-2">
            <TextInput
              className="flex-1"
              radius="md"
              placeholder="Search teams"
              aria-label="Filter teams"
              value={filter}
              onChange={(event) => onFilterChange(event.target.value)}
            />
            <IconButton
              size="md"
              className="h-9 w-9 rounded-[10px] border border-black/[0.08] bg-white/75 text-ui-text-primary hover:border-ui-border-emphasis hover:bg-ui-surface-soft"
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
          <div className="flex flex-col gap-0.5">
            {!hasTeams && (
              <p className={`${bodyTextClassName} px-2`}>
                No teams yet. Create one to open its workspace.
              </p>
            )}
            {hasTeams && items.length === 0 && (
              <p className={`${bodyTextClassName} px-2`}>No teams match the current filter.</p>
            )}
            {items.map((team) => (
              <SelectableListItem
                key={team.id}
                layout="row"
                type="button"
                className="min-w-0 justify-between gap-3 rounded-[10px] border-transparent bg-transparent px-2 py-1.5 shadow-none hover:bg-[rgba(55,53,47,0.05)]"
                data-team-selector-entry="true"
                data-team-id={team.id}
                data-team-name={team.name}
                onClick={() => onSelectTeam(team.id)}
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] font-medium text-ui-text-primary">
                    {team.name}
                  </div>
                  <div className="mt-0.5 line-clamp-1 text-[11px] leading-5 text-ui-text-secondary">
                    {team.description}
                  </div>
                  <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[10px] leading-4 text-ui-text-muted">
                    <span className="truncate">{team.summary}</span>
                    <span className="inline-flex h-1 w-1 shrink-0 rounded-full bg-ui-text-muted/35" />
                    <span className="shrink-0 capitalize">{team.runtimeLabel}</span>
                  </div>
                </div>
              </SelectableListItem>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
});
