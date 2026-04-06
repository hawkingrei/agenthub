import React from "react";
import { Loader } from "@mantine/core";
import { ActionButton } from "../ui/primitives";
import {
  TEAM_SECTION_CARD_LARGE_CLASS,
  TEAM_WORKBENCH_ACCENT_BUTTON_CLASS,
  TEAM_WORKBENCH_BADGE_CLASS,
  TEAM_WORKBENCH_PANEL_CLASS,
} from "../ui/tailwind_classes";

export function TeamLoadingPanel() {
  return (
    <div
      className={`${TEAM_SECTION_CARD_LARGE_CLASS} ${TEAM_WORKBENCH_PANEL_CLASS}`}
      data-team-loading-shell="true"
    >
      <span className={TEAM_WORKBENCH_BADGE_CLASS}>Loading Team</span>
      <div className="mt-3 flex items-center gap-3">
        <Loader size="sm" color="gray" />
        <div className="min-w-0">
          <h2 className="text-[22px] font-semibold tracking-tight text-black">
            Loading team workspace...
          </h2>
          <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/75">
            AgentHub is loading the team catalog and workspace context. The workbench
            will render once the basic Team metadata is ready.
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
    <div className={`${TEAM_SECTION_CARD_LARGE_CLASS} ${TEAM_WORKBENCH_PANEL_CLASS}`}>
      <span className={TEAM_WORKBENCH_BADGE_CLASS}>Team Not Found</span>
      <h2 className="mt-2 text-[22px] font-semibold tracking-tight text-black">
        This team is unavailable.
      </h2>
      <p className="mt-2 max-w-2xl text-[13px] leading-5 text-black/75">
        The requested team could not be loaded. Return to the selector to choose another
        team or create a new one.
      </p>
      <div className="mt-4">
        <ActionButton
          className={TEAM_WORKBENCH_ACCENT_BUTTON_CLASS}
          onClick={onBackToSelector}
        >
          Back to Team Selector
        </ActionButton>
      </div>
    </div>
  );
}
