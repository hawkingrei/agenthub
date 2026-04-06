import React from "react";
import { ActionButton } from "../ui/primitives";
import {
  TEAM_CREATE_PANEL_CARD_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS,
  TEAM_WORKBENCH_PANEL_CLASS,
} from "../ui/tailwind_classes";

const TEAM_WORKBENCH_ACCENT_BUTTON_CLASS =
  "!bg-notion-accent !text-white !border-transparent hover:!bg-notion-accent/90 transition shadow-sm active:!translate-y-px";
const TEAM_WORKBENCH_BADGE_CLASS =
  "inline-flex items-center rounded-md border border-notion-border bg-notion-sidebar px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted transition hover:bg-notion-hover";
const TEAM_WORKBENCH_SETUP_CHECKLIST_CLASS =
  "overflow-hidden rounded-xl border border-notion-border bg-white shadow-md";
const TEAM_WORKBENCH_INFO_STRIP_GRID_CLASS = "grid gap-px bg-notion-border lg:grid-cols-3";

type TeamSetupPanelProps = {
  description: string | null | undefined;
  forgeLabel: string;
  onForge: () => void;
};

export function TeamSetupPanel({
  description,
  forgeLabel,
  onForge,
}: TeamSetupPanelProps) {
  return (
    <div className={`${TEAM_CREATE_PANEL_CARD_CLASS} ${TEAM_WORKBENCH_PANEL_CLASS}`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <span className={TEAM_WORKBENCH_BADGE_CLASS}>Team Setup</span>
          <h3 className="mt-2 text-[18px] font-semibold tracking-tight text-black">
            No agents have joined this team yet.
          </h3>
          <p className="mt-2 max-w-2xl text-[13px] leading-5 text-ui-text-secondary">
            The team goal is saved, but runtime and runs stay blocked until you add the
            first agent.
          </p>
        </div>
        <ActionButton
          className={TEAM_WORKBENCH_ACCENT_BUTTON_CLASS}
          onClick={onForge}
        >
          <i className="bi bi-person-plus" aria-hidden="true" />
          <span>{forgeLabel}</span>
        </ActionButton>
      </div>
      <div className={`${TEAM_WORKBENCH_SETUP_CHECKLIST_CLASS} mt-4`}>
        <div className={TEAM_WORKBENCH_INFO_STRIP_GRID_CLASS}>
          <div className={TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS}>
            <p className={TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS}>Goal</p>
            <p className={TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS}>
              {description?.trim() ||
                "Capture the mission, constraints, and what this team should own."}
            </p>
          </div>
          <div className={TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS}>
            <p className={TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS}>First Agent</p>
            <p className={TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS}>
              Add the first agent with identity, skills, prompt, and workdir.
            </p>
          </div>
          <div className={TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS}>
            <p className={TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS}>Unlocks</p>
            <p className={TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS}>
              Runtime, runs, and shared execution views unlock automatically once an
              agent exists.
            </p>
          </div>
        </div>
        <div className="grid gap-px border-t border-ui-border bg-ui-border lg:grid-cols-3">
          <div className={TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS}>
            <p className={TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS}>Step 1</p>
            <p className={TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS}>Create the first agent</p>
          </div>
          <div className={TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS}>
            <p className={TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS}>Step 2</p>
            <p className={TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS}>Add more agents</p>
          </div>
          <div className={TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS}>
            <p className={TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS}>Step 3</p>
            <p className={TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS}>Start runtime and runs</p>
          </div>
        </div>
      </div>
    </div>
  );
}
