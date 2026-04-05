import React from "react";
import {
  REQUEST_USER_INPUT_OTHER_OPTION_LABEL,
  buildRequestUserInputSubmissionText,
  countAnsweredRequestUserInputQuestions,
  createInitialRequestUserInputDrafts,
  splitRequestUserInputAnswer,
  type RequestUserInputDrafts,
  type RequestUserInputQuestion,
  type RequestUserInputResponse,
} from "../request_user_input";
import { ACP_SEGMENTED_NOTE_WARNING_CLASS } from "../ui/tailwind_classes";

const REQUEST_USER_INPUT_SUBMIT_BUTTON_CLASS =
  "inline-flex h-9 items-center justify-center rounded-md bg-notion-accent px-4 text-[13px] font-bold text-white shadow-sm transition hover:bg-notion-accent/90 disabled:opacity-50 active:translate-y-px";

export function RequestUserInputCard({
  toolCallId,
  questions,
  canSubmit,
  onSubmitRequestUserInput,
}: {
  toolCallId: string;
  questions: RequestUserInputQuestion[];
  canSubmit: boolean;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
}) {
  const [drafts, setDrafts] = React.useState<RequestUserInputDrafts>(() =>
    createInitialRequestUserInputDrafts(questions)
  );
  const [submitting, setSubmitting] = React.useState(false);
  const [errorText, setErrorText] = React.useState<string | null>(null);
  const questionsResetKey = createRequestUserInputQuestionsResetKey(questions);
  const resetStateKey = `${toolCallId}::${questionsResetKey}`;
  const lastResetStateKeyRef = React.useRef<string | null>(null);

  React.useEffect(() => {
    if (lastResetStateKeyRef.current === resetStateKey) {
      return;
    }
    lastResetStateKeyRef.current = resetStateKey;
    setDrafts(createInitialRequestUserInputDrafts(questions));
    setSubmitting(false);
    setErrorText(null);
  }, [questions, resetStateKey]);

  const handleOptionChange = React.useCallback(
    (questionId: string, optionLabel: string) => {
      setDrafts((prev) => ({
        ...prev,
        [questionId]: {
          selectedOptionLabel: optionLabel,
          note: prev[questionId]?.note ?? "",
        },
      }));
      setErrorText(null);
    },
    []
  );

  const handleNoteChange = React.useCallback((questionId: string, note: string) => {
    setDrafts((prev) => ({
      ...prev,
      [questionId]: {
        selectedOptionLabel: prev[questionId]?.selectedOptionLabel ?? null,
        note,
      },
    }));
    setErrorText(null);
  }, []);

  const handleSubmit = React.useCallback(async () => {
    if (!onSubmitRequestUserInput) {
      return;
    }
    const submission = buildRequestUserInputSubmissionText(questions, drafts);
    if (!submission.text) {
      setErrorText("Answer required before continuing.");
      return;
    }
    if (submission.missingQuestionIds.length > 0) {
      setErrorText(`Answer required for: ${submission.missingQuestionIds.join(", ")}.`);
      return;
    }

    try {
      setSubmitting(true);
      setErrorText(null);
      await onSubmitRequestUserInput(submission.text);
    } catch (error) {
      setErrorText(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  }, [drafts, onSubmitRequestUserInput, questions]);

  return (
    <div className="mx-0 mb-3 mt-2 rounded-xl border border-notion-border bg-notion-sidebar/30 p-4 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <div className="text-sm font-bold text-notion-text">Input Required</div>
          <div className="text-xs text-notion-text-muted">
            Submit your answer to continue execution.
          </div>
        </div>
        <span className="rounded-sm bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted">
          Pending
        </span>
      </div>
      <div className="mt-4 space-y-4">
        {questions.map((question, index) => {
          const draft = drafts[question.id] ?? {
            selectedOptionLabel: null,
            note: "",
          };
          const hasOptions = question.options != null && question.options.length > 0;
          const questionHeaderId = `${toolCallId}:${question.id}:header`;
          const questionPromptId = `${toolCallId}:${question.id}:prompt`;
          const questionTextareaId = `${toolCallId}:${question.id}:note`;
          return (
            <div
              key={question.id}
              className="rounded-lg border border-notion-border bg-white p-4 shadow-sm"
              data-request-user-input-question={question.id}
            >
              <div className="flex flex-wrap items-center gap-2">
                <span className="rounded-md bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted">
                  {questions.length > 1 ? `Q${index + 1}` : "Question"}
                </span>
                <span id={questionHeaderId} className="text-sm font-bold text-notion-text">
                  {question.header || question.id}
                </span>
                {question.isSecret ? (
                  <span className="rounded-md bg-rose-50 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-rose-600">
                    Secret
                  </span>
                ) : null}
              </div>
              <p
                id={questionPromptId}
                className="mt-2 text-[14px] leading-relaxed text-notion-text"
              >
                {question.question}
              </p>
              {hasOptions ? (
                <div className="mt-4 space-y-2">
                  {question.options?.map((option) => {
                    const checked = draft.selectedOptionLabel === option.label;
                    return (
                      <label
                        key={option.label}
                        className={`flex cursor-pointer items-start gap-3 rounded-md border p-3 transition ${
                          checked
                            ? "border-notion-accent bg-notion-accent-bg"
                            : "border-notion-border bg-white hover:bg-notion-hover"
                        }`}
                      >
                        <input
                          type="radio"
                          name={`${toolCallId}:${question.id}`}
                          value={option.label}
                          checked={checked}
                          onChange={() => handleOptionChange(question.id, option.label)}
                          disabled={submitting}
                          data-request-user-input-option={option.label}
                        />
                        <span className="min-w-0">
                          <span className="block text-sm font-bold text-notion-text">
                            {option.label}
                          </span>
                          <span className="mt-0.5 block text-xs leading-relaxed text-notion-text-muted">
                            {option.description}
                          </span>
                        </span>
                      </label>
                    );
                  })}
                  {question.isOther ? (
                    <label
                      className={`flex cursor-pointer items-start gap-3 rounded-md border p-3 transition ${
                        draft.selectedOptionLabel === REQUEST_USER_INPUT_OTHER_OPTION_LABEL
                          ? "border-notion-accent bg-notion-accent-bg"
                          : "border-notion-border bg-white hover:bg-notion-hover"
                      }`}
                    >
                      <input
                        type="radio"
                        name={`${toolCallId}:${question.id}`}
                        value={REQUEST_USER_INPUT_OTHER_OPTION_LABEL}
                        checked={
                          draft.selectedOptionLabel === REQUEST_USER_INPUT_OTHER_OPTION_LABEL
                        }
                        onChange={() =>
                          handleOptionChange(question.id, REQUEST_USER_INPUT_OTHER_OPTION_LABEL)
                        }
                        disabled={submitting}
                        data-request-user-input-option={REQUEST_USER_INPUT_OTHER_OPTION_LABEL}
                      />
                      <span className="min-w-0">
                        <span className="block text-sm font-bold text-notion-text">
                          {REQUEST_USER_INPUT_OTHER_OPTION_LABEL}
                        </span>
                        <span className="mt-0.5 block text-xs leading-relaxed text-notion-text-muted">
                          Provide custom input in the field below.
                        </span>
                      </span>
                    </label>
                  ) : null}
                </div>
              ) : null}
              <textarea
                id={questionTextareaId}
                className="mono mt-4 min-h-24 w-full rounded-md border border-notion-border bg-white px-3 py-2 text-[13px] text-notion-text outline-none transition focus:border-notion-accent focus:ring-2 focus:ring-notion-accent/10"
                name={questionTextareaId}
                aria-labelledby={`${questionHeaderId} ${questionPromptId}`}
                value={draft.note}
                onChange={(event) => handleNoteChange(question.id, event.currentTarget.value)}
                placeholder={
                  hasOptions
                    ? question.isOther
                      ? "Custom answer or details..."
                      : "Optional notes..."
                    : "Type your answer..."
                }
                disabled={submitting}
                data-request-user-input-note={question.id}
              />
              {question.isSecret ? (
                <div className={`${ACP_SEGMENTED_NOTE_WARNING_CLASS} mt-3`}>
                  Secret answers are submitted but not persisted in history.
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
      {errorText ? (
        <div className={`${ACP_SEGMENTED_NOTE_WARNING_CLASS} mt-4`}>{errorText}</div>
      ) : null}
      <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
        <p className="max-w-sm text-[11px] italic leading-relaxed text-notion-text-muted">
          Response will be sent through the active turn.
        </p>
        <button
          type="button"
          className={REQUEST_USER_INPUT_SUBMIT_BUTTON_CLASS}
          onClick={() => {
            void handleSubmit();
          }}
          disabled={!canSubmit || submitting}
          data-request-user-input-submit={toolCallId}
        >
          {submitting ? "Submitting..." : "Submit Answer"}
        </button>
      </div>
    </div>
  );
}

export function RequestUserInputResultCard({
  questions,
  response,
  statusLabel,
}: {
  questions: RequestUserInputQuestion[];
  response: RequestUserInputResponse | null;
  statusLabel?: string;
}) {
  const answeredCount = React.useMemo(
    () => countAnsweredRequestUserInputQuestions(questions, response),
    [questions, response]
  );
  const hasSecretQuestions = questions.some((question) => question.isSecret);
  const hideAllAnswers = hasSecretQuestions && response == null;

  return (
    <div className="mx-0 mb-3 mt-2 rounded-xl border border-notion-border bg-notion-sidebar/30 p-4 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <div className="text-sm font-bold text-notion-text">
            {questions.length === 1 ? "Question answered" : "Questions answered"}
          </div>
          <div className="text-xs text-notion-text-muted">
            {answeredCount}/{questions.length} answers recorded
            {statusLabel ? ` · ${statusLabel}` : ""}
          </div>
        </div>
        <span className="rounded-sm bg-emerald-50 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-emerald-700">
          Complete
        </span>
      </div>
      <div className="mt-4 space-y-4">
        {questions.map((question, index) => (
          <RequestUserInputResultQuestion
            key={question.id}
            question={question}
            index={index}
            totalQuestions={questions.length}
            answer={response?.answers[question.id]}
            hideAnswer={hideAllAnswers || question.isSecret}
          />
        ))}
      </div>
      {hideAllAnswers ? (
        <div className={`${ACP_SEGMENTED_NOTE_WARNING_CLASS} mt-4`}>
          Agent suppressed the structured answer payload in execution history.
        </div>
      ) : null}
    </div>
  );
}

function createRequestUserInputQuestionsResetKey(
  questions: RequestUserInputQuestion[]
): string {
  return JSON.stringify(
    questions.map((question) => ({
      id: question.id,
      header: question.header ?? null,
      question: question.question,
      isOther: question.isOther,
      isSecret: question.isSecret,
      options:
        question.options?.map((option) => ({
          label: option.label,
          description: option.description,
        })) ?? null,
    }))
  );
}

function RequestUserInputResultQuestion({
  question,
  index,
  totalQuestions,
  answer,
  hideAnswer,
}: {
  question: RequestUserInputQuestion;
  index: number;
  totalQuestions: number;
  answer: RequestUserInputResponse["answers"][string] | undefined;
  hideAnswer: boolean;
}) {
  const parts = splitRequestUserInputAnswer(answer);
  const hasOptions = question.options != null && question.options.length > 0;
  const hasStructuredAnswer =
    parts.options.length > 0 || (parts.note != null && parts.note.length > 0);

  return (
    <div
      className="rounded-lg border border-notion-border bg-white p-4 shadow-sm"
      data-request-user-input-result={question.id}
    >
      <div className="flex flex-wrap items-center gap-2">
        <span className="rounded-md bg-notion-hover px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-notion-text-muted">
          {totalQuestions > 1 ? `Q${index + 1}` : "Question"}
        </span>
        <span className="text-sm font-bold text-notion-text">
          {question.header || question.id}
        </span>
        {question.isSecret ? (
          <span className="rounded-md bg-rose-50 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-rose-600">
            Secret
          </span>
        ) : null}
      </div>
      <p className="mt-2 text-[14px] leading-relaxed text-notion-text">{question.question}</p>
      {hideAnswer ? (
        <div className="mt-3 rounded-md border border-state-warning-border bg-state-warning-bg px-3 py-2 text-[12px] italic text-state-warning-text">
          Answer submitted privately.
        </div>
      ) : hasStructuredAnswer ? (
        <div className="mt-4 space-y-3">
          {hasOptions ? (
            <div className="flex flex-wrap gap-2">
              {parts.options.map((entry) => (
                <span
                  key={entry}
                  className="rounded-md border border-notion-accent/10 bg-notion-accent-bg px-2 py-0.5 text-[12px] font-bold text-notion-accent"
                >
                  {entry}
                </span>
              ))}
            </div>
          ) : (
            <div className="space-y-2">
              {parts.options.map((entry) => (
                <div
                  key={entry}
                  className="mono rounded-md border border-notion-border bg-notion-sidebar/20 px-3 py-2 text-[13px] text-notion-text"
                >
                  {entry}
                </div>
              ))}
            </div>
          )}
          {parts.note ? (
            <div className="mono rounded-md border border-notion-border bg-notion-sidebar/20 px-3 py-2 text-[13px] text-notion-text">
              {parts.note}
            </div>
          ) : null}
        </div>
      ) : (
        <div className="mt-3 rounded-md border border-notion-border bg-notion-sidebar/10 px-3 py-2 text-[12px] italic text-notion-text-muted">
          No payload recorded.
        </div>
      )}
    </div>
  );
}
