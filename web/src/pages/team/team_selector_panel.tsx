import React from "react";
import { Button, TextInput, UnstyledButton } from "@mantine/core";
import { IconButton } from "../../ui/primitives";

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
      <section className="flex min-h-0 w-full max-w-[720px] flex-col">
        <div className="flex items-start justify-between gap-3 px-2">
          <div className="min-w-0 flex-1">
            <h2 className="text-[18px] font-semibold tracking-tight text-black">Teams</h2>
          </div>
          <Button type="button" radius="md" className={accentButtonClassName} onClick={onCreateTeam}>
            Create Team
          </Button>
        </div>

        {hasTeams && (
          <div className="mt-3 flex items-center gap-2 px-2">
            <TextInput
              className="flex-1"
              radius="md"
              placeholder="Filter teams by name or id"
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
          </div>
        )}

        <div className="mt-3 flex-1 overflow-y-auto">
          <div className="flex flex-col gap-0.5">
            {!hasTeams && (
              <p className={`${bodyTextClassName} px-2`}>
                No teams yet. Create the team first, then enter its workspace to add agents.
              </p>
            )}
            {hasTeams && items.length === 0 && (
              <p className={`${bodyTextClassName} px-2`}>No teams match the current filter.</p>
            )}
            {items.map((team) => (
              <UnstyledButton
                key={team.id}
                type="button"
                className="team-item flex w-full min-w-0 items-start justify-between gap-3 rounded-[10px] border border-transparent bg-transparent px-2 py-2 text-left text-ui-text-primary transition hover:bg-[rgba(55,53,47,0.05)]"
                data-team-selector-entry="true"
                data-team-id={team.id}
                data-team-name={team.name}
                onClick={() => onSelectTeam(team.id)}
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[14px] font-semibold text-ui-text-primary">
                    {team.name}
                  </div>
                  <div className="mt-1 line-clamp-1 text-[12px] leading-5 text-ui-text-secondary">
                    {team.description}
                  </div>
                  <div className="mt-1 text-[11px] leading-4 text-ui-text-muted">{team.summary}</div>
                </div>
                <div className="mt-0.5 shrink-0 text-[10px] font-medium uppercase tracking-[0.12em] text-ui-text-muted">
                  {team.runtimeLabel}
                </div>
              </UnstyledButton>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
});
