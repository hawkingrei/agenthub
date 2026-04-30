import { ActionButton, PanelHeader } from "../ui/primitives";
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
  onForge: () => void;
};

export function TeamSetupPanel({
  description,
  forgeLabel,
  onForge,
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
        subtitle="The team goal is saved, but runtime and runs stay blocked until you add the first agent."
        className="border-b-0 pb-0"
        titleClassName="text-base font-normal"
        subtitleClassName="max-w-2xl text-[13px] leading-5 text-ui-text-secondary"
        contentClassName="gap-0"
        actions={
          <ActionButton
            className={TEAM_WORKBENCH_ACCENT_BUTTON_CLASS}
            onClick={onForge}
          >
            <i className="bi bi-person-plus" aria-hidden="true" />
            <span>{forgeLabel}</span>
          </ActionButton>
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
              Add the first agent with a role, a short description, and a workspace.
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
