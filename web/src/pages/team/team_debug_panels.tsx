import React from "react";
import { TextInput, Textarea } from "@mantine/core";
import {
  ActionButton,
  SurfaceCard,
  ToolbarRow,
} from "../../ui/primitives";

export type TeamDebugTag = "run_ops" | "step_ops" | "mailbox_raw";

type TeamDebugChrome = {
  panelCardClassName: string;
  sectionHeadingClassName: string;
  sectionBodyTextClassName: string;
  sectionHintTextClassName: string;
  debugTabsClassName: string;
  debugTabActiveClassName: string;
  debugTabIdleClassName: string;
  panelSecondaryButtonClassName: string;
};

type TeamDebugToolsHeaderProps = {
  chrome: TeamDebugChrome;
  teamDebugTag: TeamDebugTag;
  onTeamDebugTagChange: (tag: TeamDebugTag) => void;
};

export const TeamDebugToolsHeader = React.memo(function TeamDebugToolsHeader({
  chrome,
  teamDebugTag,
  onTeamDebugTagChange,
}: TeamDebugToolsHeaderProps) {
  return (
    <SurfaceCard className={`${chrome.panelCardClassName} p-3`}>
      <ToolbarRow>
        <h3 className={chrome.sectionHeadingClassName}>Debug Tools</h3>
        <div className={chrome.debugTabsClassName}>
          <ActionButton
            type="button"
            tone="ghost"
            size="sm"
            className={
              teamDebugTag === "run_ops"
                ? chrome.debugTabActiveClassName
                : chrome.debugTabIdleClassName
            }
            onClick={() => onTeamDebugTagChange("run_ops")}
          >
            Run Ops
          </ActionButton>
          <ActionButton
            type="button"
            tone="ghost"
            size="sm"
            className={
              teamDebugTag === "step_ops"
                ? chrome.debugTabActiveClassName
                : chrome.debugTabIdleClassName
            }
            onClick={() => onTeamDebugTagChange("step_ops")}
          >
            Step Ops
          </ActionButton>
          <ActionButton
            type="button"
            tone="ghost"
            size="sm"
            className={
              teamDebugTag === "mailbox_raw"
                ? chrome.debugTabActiveClassName
                : chrome.debugTabIdleClassName
            }
            onClick={() => onTeamDebugTagChange("mailbox_raw")}
          >
            Mailbox Raw
          </ActionButton>
        </div>
      </ToolbarRow>
    </SurfaceCard>
  );
});

type TeamRunOpsPanelProps = {
  chrome: TeamDebugChrome;
  busy: string | null;
  runContextId: string;
  runInput: string;
  runLookupId: string;
  canCreateRun: boolean;
  runInputHasError: boolean;
  runInputError: string | null;
  createRunTitle: string;
  parsedRunInput: unknown;
  helperText: string;
  onRunContextIdChange: (value: string) => void;
  onRunInputChange: (value: string) => void;
  onRunLookupIdChange: (value: string) => void;
  onCreateRun: () => void | Promise<void>;
  onLoadRunById: () => void | Promise<void>;
  onUseExampleJson: () => void;
  onSetEmptyObject: () => void;
  onFormatJson: () => void;
  onClearRunInput: () => void;
};

export const TeamRunOpsPanel = React.memo(function TeamRunOpsPanel({
  chrome,
  busy,
  runContextId,
  runInput,
  runLookupId,
  canCreateRun,
  runInputHasError,
  runInputError,
  createRunTitle,
  parsedRunInput,
  helperText,
  onRunContextIdChange,
  onRunInputChange,
  onRunLookupIdChange,
  onCreateRun,
  onLoadRunById,
  onUseExampleJson,
  onSetEmptyObject,
  onFormatJson,
  onClearRunInput,
}: TeamRunOpsPanelProps) {
  return (
    <div className="space-y-3">
      <SurfaceCard className={`${chrome.panelCardClassName} p-4`}>
        <h4 className={chrome.sectionHeadingClassName}>Create Run</h4>
        <p className={chrome.sectionBodyTextClassName}>
          Debug entry for manually starting a Team run.
        </p>
        <ToolbarRow className="mt-3 flex-col sm:flex-row sm:items-start">
          <TextInput
            className="flex-1"
            radius="md"
            placeholder="context_id (optional, auto-generated when empty)"
            value={runContextId}
            onChange={(event) => onRunContextIdChange(event.target.value)}
          />
          <ActionButton
            tone="primary"
            size="md"
            onClick={() => void onCreateRun()}
            disabled={!canCreateRun}
            title={createRunTitle}
          >
            Create Run
          </ActionButton>
        </ToolbarRow>
        <p className={chrome.sectionHintTextClassName}>
          <code>context_id</code> can be empty. Use one when you want retries/resume grouped
          under the same context.
        </p>
        <Textarea
          className="mt-3"
          radius="md"
          minRows={8}
          autosize
          placeholder='Optional JSON input, e.g. {"task":"sync"}'
          aria-label="Run input JSON"
          spellCheck={false}
          value={runInput}
          onChange={(event) => onRunInputChange(event.target.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && canCreateRun) {
              event.preventDefault();
              void onCreateRun();
            }
          }}
          styles={{ input: { fontFamily: "monospace", fontSize: "12px", lineHeight: "1.5" } }}
        />
        {runInputError ? (
          <p className="mt-2 text-xs text-rose-600" role="alert">
            {runInputError}
          </p>
        ) : (
          <p className={chrome.sectionHintTextClassName}>{helperText}</p>
        )}
        <ToolbarRow className="mt-2 justify-start gap-2">
          <ActionButton
            type="button"
            tone="secondary"
            size="sm"
            onClick={onUseExampleJson}
          >
            Use Example JSON
          </ActionButton>
          <ActionButton
            type="button"
            tone="secondary"
            size="sm"
            onClick={onSetEmptyObject}
          >
            Set Empty Object
          </ActionButton>
          <ActionButton
            type="button"
            tone="secondary"
            size="sm"
            onClick={onFormatJson}
            disabled={runInputHasError || parsedRunInput === undefined}
          >
            Format JSON
          </ActionButton>
          <ActionButton
            type="button"
            tone="secondary"
            size="sm"
            onClick={onClearRunInput}
            disabled={runInput.trim().length === 0}
          >
            Clear
          </ActionButton>
        </ToolbarRow>
        <p className={chrome.sectionHintTextClassName}>
          Leave empty to submit default empty input <code>{`{}`}</code>.
        </p>
      </SurfaceCard>
      <SurfaceCard className={`${chrome.panelCardClassName} p-4`}>
        <h4 className={chrome.sectionHeadingClassName}>Load Existing Run</h4>
        <p className={chrome.sectionBodyTextClassName}>
          Load by <code>run_id</code> for the currently selected team only.
        </p>
        <ToolbarRow className="mt-3 flex-col sm:flex-row sm:items-start">
          <TextInput
            className="flex-1"
            radius="md"
            placeholder="existing run_id"
            value={runLookupId}
            onChange={(event) => onRunLookupIdChange(event.target.value)}
          />
          <ActionButton
            tone="secondary"
            size="md"
            onClick={() => void onLoadRunById()}
            disabled={busy === "load-run"}
          >
            {busy === "load-run" ? "Loading..." : "Load Run"}
          </ActionButton>
        </ToolbarRow>
      </SurfaceCard>
    </div>
  );
});

type TeamRunRequiredPanelProps = {
  chrome: TeamDebugChrome;
  title: string;
  body: string;
  onGoToRuns: () => void;
};

export const TeamRunRequiredPanel = React.memo(function TeamRunRequiredPanel({
  chrome,
  title,
  body,
  onGoToRuns,
}: TeamRunRequiredPanelProps) {
  return (
    <SurfaceCard className={chrome.panelCardClassName}>
      <h4 className={chrome.sectionHeadingClassName}>{title}</h4>
      <p className={chrome.sectionBodyTextClassName}>{body}</p>
      <div className="mt-3">
        <ActionButton
          tone="secondary"
          size="md"
          className={chrome.panelSecondaryButtonClassName}
          onClick={onGoToRuns}
        >
          Go to Runs
        </ActionButton>
      </div>
    </SurfaceCard>
  );
});
