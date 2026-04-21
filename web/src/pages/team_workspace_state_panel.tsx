import React from "react";
import { Loader } from "@mantine/core";
import { ActionButton } from "../ui/primitives";

export function TeamLoadingPanel() {
  return (
    <div className="mx-auto flex w-full max-w-[680px] flex-col px-6 py-10" data-team-loading-shell="true">
      <div className="flex items-center gap-3">
        <Loader size="sm" color="gray" />
        <div className="min-w-0">
          <p className="text-[11px] font-medium tracking-[0.01em] text-black/45">Loading</p>
          <h2 className="mt-1 text-[20px] font-semibold tracking-tight text-black">
            Loading team workspace
          </h2>
          <p className="mt-2 max-w-xl text-[13px] leading-5 text-black/65">
            AgentHub is loading the workspace context and team metadata.
          </p>
        </div>
      </div>
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
      <p className="text-[11px] font-medium tracking-[0.01em] text-black/45">Unavailable</p>
      <h2 className="mt-1 text-[20px] font-semibold tracking-tight text-black">
        This team is unavailable
      </h2>
      <p className="mt-2 max-w-xl text-[13px] leading-5 text-black/65">
        The requested team could not be loaded. Return to the team list and choose another one.
      </p>
      <div className="mt-5">
        <ActionButton
          tone="ghost"
          className="h-8 justify-start rounded-md px-0 text-[12px] font-medium text-notion-text-muted hover:text-notion-text"
          onClick={onBackToSelector}
        >
          Back to teams
        </ActionButton>
      </div>
    </div>
  );
}
