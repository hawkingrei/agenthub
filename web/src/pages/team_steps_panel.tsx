import React from "react";
import { TeamStepRecord } from "../api";
import {
  ActionButton,
  EmptyState,
  InlineNotice,
  InsetSurface,
  KeyValueItem,
  KeyValueList,
  SurfaceCard,
  ToolbarRow,
} from "../ui/primitives";
import {
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type StepAction = "start" | "complete" | "fail" | "input_required" | "resume";

type TeamStepsPanelProps = {
  steps: TeamStepRecord[];
  developerMode: boolean;
  mode?: "full" | "list_only" | "controls_only";
  onRefreshSteps: () => Promise<void> | void;
  stepKey: string;
  onStepKeyChange: (value: string) => void;
  stepMemberId: string;
  onStepMemberIdChange: (value: string) => void;
  stepDependsOn: string;
  onStepDependsOnChange: (value: string) => void;
  stepInput: string;
  onStepInputChange: (value: string) => void;
  onSubmitStep: () => Promise<void> | void;
  busy: string | null;
  selectedStepId: string;
  onSelectedStepIdChange: (value: string) => void;
  stepAction: StepAction;
  onStepActionChange: (value: StepAction) => void;
  stepRemoteTaskId: string;
  onStepRemoteTaskIdChange: (value: string) => void;
  stepOutput: string;
  onStepOutputChange: (value: string) => void;
  stepFailText: string;
  onStepFailTextChange: (value: string) => void;
  stepInputReason: string;
  onStepInputReasonChange: (value: string) => void;
  stepInputRequiredPayload: string;
  onStepInputRequiredPayloadChange: (value: string) => void;
  stepResumePayload: string;
  onStepResumePayloadChange: (value: string) => void;
  onApplyStepAction: () => Promise<void> | void;
};

const STEPS_PANEL_CLASS =
  "teams-step-panel min-w-0 rounded-xl border border-ui-border bg-ui-surface-soft/70 p-3";
const STEPS_LIST_CLASS =
  "teams-step-list m-0 flex max-h-[420px] list-none flex-col gap-2 overflow-auto rounded-xl border border-ui-border bg-ui-surface-soft/60 p-3";
const STEPS_GRID_CLASS = "teams-step-grid grid gap-3 lg:grid-cols-2";
const STEPS_PANEL_TITLE_CLASS = "mb-2 text-ui-sm font-semibold text-ui-text-primary";
const STEPS_ITEM_CLASS = "rounded-lg border border-ui-border bg-ui-surface p-2";
const STEPS_ITEM_HEAD_CLASS =
  "teams-step-head mb-1 flex flex-wrap items-center gap-2 text-ui-xs text-ui-text-muted";
const STEPS_ITEM_BODY_CLASS =
  "teams-step-body mono flex flex-col gap-1 text-ui-xs text-ui-text-muted break-words";
const STEPS_LIST_ONLY_NOTE_CLASS =
  "mb-3 rounded-lg border border-state-warning-border bg-state-warning-bg px-3 py-2 text-ui-sm text-state-warning-text";

function TeamStepsPanelImpl(props: TeamStepsPanelProps) {
  const {
    steps,
    developerMode,
    mode = "full",
    onRefreshSteps,
    stepKey,
    onStepKeyChange,
    stepMemberId,
    onStepMemberIdChange,
    stepDependsOn,
    onStepDependsOnChange,
    stepInput,
    onStepInputChange,
    onSubmitStep,
    busy,
    selectedStepId,
    onSelectedStepIdChange,
    stepAction,
    onStepActionChange,
    stepRemoteTaskId,
    onStepRemoteTaskIdChange,
    stepOutput,
    onStepOutputChange,
    stepFailText,
    onStepFailTextChange,
    stepInputReason,
    onStepInputReasonChange,
    stepInputRequiredPayload,
    onStepInputRequiredPayloadChange,
    stepResumePayload,
    onStepResumePayloadChange,
    onApplyStepAction,
  } = props;
  const showControls = mode !== "list_only";
  const showList = mode !== "controls_only";

  return (
    <SurfaceCard className="p-4">
      <ToolbarRow className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Steps</h3>
        <ActionButton
          onClick={() => {
            void onRefreshSteps();
          }}
          tone="secondary"
          size="md"
          title="Refresh steps"
          aria-label="Refresh steps"
        >
          <i className="bi bi-arrow-clockwise" aria-hidden="true" />
          <span>Refresh</span>
        </ActionButton>
      </ToolbarRow>

      {showControls && (
        <div className={STEPS_GRID_CLASS}>
          <InsetSurface className={STEPS_PANEL_CLASS}>
            <h4 className={STEPS_PANEL_TITLE_CLASS}>Submit Step</h4>
            <input
              className={TEAM_PANEL_INPUT_CLASS}
              placeholder="step_key"
              value={stepKey}
              onChange={(event) => onStepKeyChange(event.target.value)}
            />
            <input
              className={TEAM_PANEL_INPUT_CLASS}
              placeholder="member_id"
              value={stepMemberId}
              onChange={(event) => onStepMemberIdChange(event.target.value)}
            />
            <input
              className={TEAM_PANEL_INPUT_CLASS}
              placeholder="depends_on (comma separated)"
              value={stepDependsOn}
              onChange={(event) => onStepDependsOnChange(event.target.value)}
            />
            <textarea
              className={TEAM_PANEL_TEXTAREA_CLASS}
              rows={4}
              value={stepInput}
              onChange={(event) => onStepInputChange(event.target.value)}
            />
            <ActionButton
              onClick={() => {
                void onSubmitStep();
              }}
              disabled={busy === "submit-step"}
              tone="primary"
              size="md"
            >
              Submit Step
            </ActionButton>
          </InsetSurface>

          <InsetSurface className={STEPS_PANEL_CLASS}>
            <h4 className={STEPS_PANEL_TITLE_CLASS}>Step Action</h4>
            <select
              className={TEAM_PANEL_INPUT_CLASS}
              value={selectedStepId}
              onChange={(event) => onSelectedStepIdChange(event.target.value)}
            >
              <option value="">Select step</option>
              {steps.map((step) => (
                <option key={step.id} value={step.id}>
                  {step.step_key} ({step.status})
                </option>
              ))}
            </select>
            <select
              className={TEAM_PANEL_INPUT_CLASS}
              value={stepAction}
              onChange={(event) => onStepActionChange(event.target.value as StepAction)}
            >
              <option value="start">start</option>
              <option value="complete">complete</option>
              <option value="fail">fail</option>
              <option value="input_required">input_required</option>
              <option value="resume">resume</option>
            </select>

            {stepAction === "start" && (
              <input
                className={TEAM_PANEL_INPUT_CLASS}
                placeholder="runtime_handle_id (optional)"
                value={stepRemoteTaskId}
                onChange={(event) => onStepRemoteTaskIdChange(event.target.value)}
              />
            )}

            {stepAction === "complete" && (
              <textarea
                className={TEAM_PANEL_TEXTAREA_CLASS}
                rows={4}
                value={stepOutput}
                onChange={(event) => onStepOutputChange(event.target.value)}
              />
            )}

            {stepAction === "fail" && (
              <input
                className={TEAM_PANEL_INPUT_CLASS}
                placeholder="error_text"
                value={stepFailText}
                onChange={(event) => onStepFailTextChange(event.target.value)}
              />
            )}

            {stepAction === "input_required" && (
              <>
                <input
                  className={TEAM_PANEL_INPUT_CLASS}
                  placeholder="reason (optional)"
                  value={stepInputReason}
                  onChange={(event) => onStepInputReasonChange(event.target.value)}
                />
                <textarea
                  className={TEAM_PANEL_TEXTAREA_CLASS}
                  rows={4}
                  value={stepInputRequiredPayload}
                  onChange={(event) => onStepInputRequiredPayloadChange(event.target.value)}
                />
              </>
            )}

            {stepAction === "resume" && (
              <textarea
                className={TEAM_PANEL_TEXTAREA_CLASS}
                rows={4}
                value={stepResumePayload}
                onChange={(event) => onStepResumePayloadChange(event.target.value)}
              />
            )}

            <ActionButton
              onClick={() => {
                void onApplyStepAction();
              }}
              tone="secondary"
              size="md"
            >
              Apply Step Action
            </ActionButton>
          </InsetSurface>
        </div>
      )}

      {mode === "list_only" && (
        <InlineNotice tone="warning" className={STEPS_LIST_ONLY_NOTE_CLASS}>
          {developerMode ? (
            <>
              Step operations were moved to <strong>Debug -&gt; Step Ops</strong>.
            </>
          ) : (
            "Step controls are available in Developer Mode."
          )}
        </InlineNotice>
      )}

      {showList && (
        steps.length === 0 ? (
          <EmptyState
            title="No steps yet"
            body="Start a run or open Debug -> Step Ops to seed execution steps."
            className="mt-3"
          />
        ) : (
          <ul className={STEPS_LIST_CLASS}>
            {steps.map((step) => (
              <li key={step.id}>
                <InsetSurface className={STEPS_ITEM_CLASS}>
                  <div className={STEPS_ITEM_HEAD_CLASS}>
                    <span className="mono">{step.id}</span>
                    <span>{step.step_key}</span>
                    <span>{step.status}</span>
                  </div>
                  <KeyValueList className={STEPS_ITEM_BODY_CLASS}>
                    <KeyValueItem label="member_id" value={step.member_id} />
                    <KeyValueItem label="attempt" value={step.attempt} />
                    <KeyValueItem
                      label="depends_on"
                      value={step.depends_on.length ? step.depends_on.join(", ") : "-"}
                    />
                    <KeyValueItem
                      label="runtime_handle_id"
                      value={step.runtime_handle_id ?? step.remote_task_id ?? "-"}
                    />
                    {step.error_text ? (
                      <KeyValueItem label="error_text" value={step.error_text} />
                    ) : null}
                  </KeyValueList>
                </InsetSurface>
              </li>
            ))}
          </ul>
        )
      )}
    </SurfaceCard>
  );
}

export const TeamStepsPanel = React.memo(TeamStepsPanelImpl);
TeamStepsPanel.displayName = "TeamStepsPanel";
