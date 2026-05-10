import { ActionButton, AlphaBadge, InsetSurface, KeyValueItem, KeyValueList, PanelHeader, SurfaceCard } from "../ui/primitives";

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
    <SurfaceCard className="p-4 sm:p-6">
      <PanelHeader
        title={
          <div className="min-w-0">
            <span className="inline-flex items-center rounded-full bg-notion-accent/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-accent">Team Setup</span>
            <h3 className="mt-2 text-[18px] font-semibold tracking-tight text-notion-text">
              No agents have joined this team yet.
            </h3>
          </div>
        }
        subtitle="The team goal is saved. Choose one of the two agent paths below; the first added agent becomes the coordinator automatically."
        className="border-b-0 pb-0"
        titleClassName="text-base font-normal"
        subtitleClassName="max-w-2xl text-[13px] leading-5 text-notion-text-muted"
        contentClassName="gap-0"
        actionsClassName="w-full min-w-0 shrink justify-start sm:w-auto sm:shrink-0 sm:justify-end"
        actions={
          <div className="grid w-full grid-cols-1 gap-2 sm:w-auto sm:grid-cols-2">
            <ActionButton
              onClick={onCopyExisting}
              tone="secondary"
              className="w-full sm:w-auto"
            >
              <i className="bi bi-copy" aria-hidden="true" />
              <span>{copyExistingLabel}</span>
              <AlphaBadge />
            </ActionButton>
            <ActionButton
              className="w-full sm:w-auto"
              tone="primary"
              onClick={onForge}
            >
              <i className="bi bi-person-plus" aria-hidden="true" />
              <span>{forgeLabel}</span>
            </ActionButton>
          </div>
        }
      />
      
      <InsetSurface className="mt-6 bg-white shadow-sm">
        <KeyValueList className="grid grid-cols-1 gap-6 sm:grid-cols-3">
          <KeyValueItem
            label="Goal"
            value={description?.trim() || "Capture the mission, constraints, and what this team should own."}
            labelClassName="text-[11px]"
            valueClassName="text-[13px]"
          />
          <KeyValueItem
            label="First Agent"
            value="Create a new Team-owned agent or copy an existing agent configuration. The first added agent becomes the coordinator."
            labelClassName="text-[11px]"
            valueClassName="text-[13px]"
          />
          <KeyValueItem
            label="Unlocks"
            value="Runtime, runs, and shared execution views unlock automatically once an agent exists."
            labelClassName="text-[11px]"
            valueClassName="text-[13px]"
          />
        </KeyValueList>

        <div className="mt-8 grid grid-cols-1 gap-4 border-t border-notion-border/40 pt-6 sm:grid-cols-3">
          <div className="flex flex-col gap-1">
            <p className="text-[10px] font-bold uppercase tracking-wider text-notion-accent">Step 1</p>
            <p className="text-[13px] font-medium text-notion-text">Create the first coordinator agent</p>
          </div>
          <div className="flex flex-col gap-1">
            <p className="text-[10px] font-bold uppercase tracking-wider text-notion-accent">Step 2</p>
            <p className="text-[13px] font-medium text-notion-text">Add more agents</p>
          </div>
          <div className="flex flex-col gap-1">
            <p className="text-[10px] font-bold uppercase tracking-wider text-notion-accent">Step 3</p>
            <p className="text-[13px] font-medium text-notion-text">Start runtime and runs</p>
          </div>
        </div>
      </InsetSurface>
    </SurfaceCard>
  );
}
