import { ActionButton, AlphaBadge, PanelHeader } from "../ui/primitives";
import {
  TEAM_CREATE_PANEL_CARD_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_ITEM_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_LABEL_CLASS,
  TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS,
  TEAM_WORKBENCH_ACCENT_BUTTON_CLASS,
  TEAM_WORKBENCH_BADGE_CLASS,
  TEAM_WORKBENCH_PANEL_CLASS,
} from "../ui/tailwind_classes";
const TEAM_WORKBENCH_SETUP_CHECKLIST_CLASS =
  "overflow-hidden rounded-xl border border-notion-border bg-white shadow-md";
const TEAM_WORKBENCH_INFO_STRIP_GRID_CLASS = "grid gap-px bg-notion-border lg:grid-cols-3";

type TeamSetupPanelProps = {
  description: string | null | undefined;
  forgeLabel: string;
  copyExistingLabel: string;
  onForge: () => void;
  onCopyExisting: () => void;
};

export function TeamSetupPanel({
  description,
  forgeLabel,
  copyExistingLabel,
  onForge,
  onCopyExisting,
}: TeamSetupPanelProps) {
  return (
    <div className={`${TEAM_CREATE_PANEL_CARD_CLASS} ${TEAM_WORKBENCH_PANEL_CLASS}`}>
      <PanelHeader
        title={
          <div className="min-w-0">
            <span className={`${TEAM_WORKBENCH_BADGE_CLASS} inline-flex`}>Team Setup</span>
            <h3 className="mt-2 text-[18px] font-semibold tracking-tight text-black">
              No agents have joined this team yet.
            </h3>
          </div>
        }
        subtitle="The team goal is saved. Choose one of the two agent paths below; the first added agent becomes the coordinator automatically."
        className="border-b-0 pb-0"
        titleClassName="text-base font-normal"
        subtitleClassName="max-w-2xl text-[13px] leading-5 text-ui-text-secondary"
        contentClassName="gap-0"
        actionsClassName="w-full min-w-0 shrink justify-start sm:w-auto sm:shrink-0 sm:justify-end"
        actions={
          <div className="grid w-full min-w-0 gap-2 sm:w-auto sm:grid-cols-2">
            <ActionButton
              className="w-full border border-ui-border bg-white text-ui-text-primary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft"
              onClick={onCopyExisting}
              tone="secondary"
            >
              <i className="bi bi-copy" aria-hidden="true" />
              <span>{copyExistingLabel}</span>
              <AlphaBadge />
            </ActionButton>
            <ActionButton
              className={`${TEAM_WORKBENCH_ACCENT_BUTTON_CLASS} w-full`}
              onClick={onForge}
            >
              <i className="bi bi-person-plus" aria-hidden="true" />
              <span>{forgeLabel}</span>
            </ActionButton>
          </div>
        }
      />
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
              Create a new Team-owned agent or copy an existing agent configuration. The first
              added agent becomes the coordinator.
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
            <p className={TEAM_WORKBENCH_INFO_STRIP_VALUE_CLASS}>
              Create the first coordinator agent
            </p>
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
