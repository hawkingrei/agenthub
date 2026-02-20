import React from "react";
import { TeamStepRecord } from "../api";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type StepAction = "start" | "complete" | "fail" | "input_required" | "resume";

type TeamStepsPanelProps = {
  steps: TeamStepRecord[];
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

const STEPS_PANEL_CLASS = "teams-step-panel rounded-xl border border-slate-200 bg-slate-50/70 p-3";
const STEPS_LIST_CLASS = "teams-step-list rounded-xl border border-slate-200 bg-slate-50/50 p-3";
const STEPS_GRID_CLASS = "teams-step-grid grid gap-3 lg:grid-cols-2";
const STEPS_PANEL_TITLE_CLASS = "mb-2 text-base font-semibold text-slate-900";
const STEPS_ITEM_CLASS = "rounded-lg border border-slate-200 bg-white p-2";

export function TeamStepsPanel(props: TeamStepsPanelProps) {
  const {
    steps,
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

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Steps</h3>
        <button
          onClick={() => {
            void onRefreshSteps();
          }}
          className="icon-button small"
          title="Refresh steps"
          aria-label="Refresh steps"
        >
          <i className="bi bi-arrow-clockwise" aria-hidden="true" />
        </button>
      </div>

      <div className={STEPS_GRID_CLASS}>
        <div className={STEPS_PANEL_CLASS}>
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
          <button
            onClick={() => {
              void onSubmitStep();
            }}
            disabled={busy === "submit-step"}
            className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
          >
            Submit Step
          </button>
        </div>

        <div className={STEPS_PANEL_CLASS}>
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
              placeholder="remote_task_id (optional)"
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

          <button
            onClick={() => {
              void onApplyStepAction();
            }}
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
          >
            Apply Step Action
          </button>
        </div>
      </div>

      <ul className={STEPS_LIST_CLASS}>
        {steps.map((step) => (
          <li key={step.id} className={STEPS_ITEM_CLASS}>
            <div className="teams-step-head">
              <span className="mono">{step.id}</span>
              <span>{step.step_key}</span>
              <span>{step.status}</span>
            </div>
            <div className="teams-step-body mono">
              <div>member_id: {step.member_id}</div>
              <div>attempt: {step.attempt}</div>
              <div>depends_on: {step.depends_on.length ? step.depends_on.join(", ") : "-"}</div>
              <div>remote_task_id: {step.remote_task_id ?? "-"}</div>
              {step.error_text && <div>error_text: {step.error_text}</div>}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
