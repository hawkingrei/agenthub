export type RequestUserInputQuestionOption = {
  label: string;
  description: string;
};

export type RequestUserInputQuestion = {
  id: string;
  header: string;
  question: string;
  isOther: boolean;
  isSecret: boolean;
  options: RequestUserInputQuestionOption[] | null;
};

export type RequestUserInputDraft = {
  selectedOptionLabel: string | null;
  note: string;
};

export type RequestUserInputDrafts = Record<string, RequestUserInputDraft>;

export type RequestUserInputAnswer = {
  answers: string[];
};

export type RequestUserInputResponse = {
  answers: Record<string, RequestUserInputAnswer>;
};

export type RequestUserInputAnswerParts = {
  options: string[];
  note: string | null;
};

type RequestUserInputAnswerValue = string | string[];

const REQUEST_USER_INPUT_TOOL_CALL_PREFIX = "request-user-input:";
export const REQUEST_USER_INPUT_OTHER_OPTION_LABEL = "None of the above";

export function parseRequestUserInputQuestions(
  toolCallId: string,
  rawInput: unknown
): RequestUserInputQuestion[] | null {
  if (!toolCallId.startsWith(REQUEST_USER_INPUT_TOOL_CALL_PREFIX)) {
    return null;
  }
  if (!Array.isArray(rawInput)) {
    return null;
  }

  const questions = rawInput
    .map(parseRequestUserInputQuestion)
    .filter((question): question is RequestUserInputQuestion => question !== null);
  return questions.length > 0 ? questions : null;
}

export function createInitialRequestUserInputDrafts(
  questions: RequestUserInputQuestion[]
): RequestUserInputDrafts {
  return Object.fromEntries(
    questions.map((question) => [
      question.id,
      {
        selectedOptionLabel: null,
        note: "",
      } satisfies RequestUserInputDraft,
    ])
  );
}

export function formatRequestUserInputSummary(
  questions: RequestUserInputQuestion[]
): string {
  return questions.length === 1 ? "1 question" : `${questions.length} questions`;
}

export function parseRequestUserInputResponse(
  rawOutput: unknown
): RequestUserInputResponse | null {
  const record = asRecord(rawOutput);
  const answersRecord = asRecord(record?.answers);
  if (!answersRecord) {
    return null;
  }

  const answers = Object.fromEntries(
    Object.entries(answersRecord)
      .map(([questionId, answerValue]) => {
        const answerRecord = asRecord(answerValue);
        const answerItems = Array.isArray(answerRecord?.answers)
          ? answerRecord.answers
              .filter((entry): entry is string => typeof entry === "string")
              .map((entry) => entry.trim())
              .filter((entry) => entry.length > 0)
          : [];
        return [questionId, { answers: answerItems } satisfies RequestUserInputAnswer] as const;
      })
      .filter(
        (
          entry
        ): entry is readonly [string, RequestUserInputAnswer] => entry[1].answers.length > 0
      )
  );

  return Object.keys(answers).length > 0 ? { answers } : null;
}

export function splitRequestUserInputAnswer(
  answer: RequestUserInputAnswer | undefined
): RequestUserInputAnswerParts {
  const parts: RequestUserInputAnswerParts = {
    options: [],
    note: null,
  };

  for (const entry of answer?.answers ?? []) {
    if (entry.startsWith("user_note: ")) {
      parts.note = entry.slice("user_note: ".length).trim() || null;
      continue;
    }
    parts.options.push(entry);
  }

  return parts;
}

export function countAnsweredRequestUserInputQuestions(
  questions: RequestUserInputQuestion[],
  response: RequestUserInputResponse | null
): number {
  if (!response) {
    return 0;
  }
  return questions.filter((question) => {
    const answer = response.answers[question.id];
    return Array.isArray(answer?.answers) && answer.answers.length > 0;
  }).length;
}

export function buildRequestUserInputSubmissionText(
  questions: RequestUserInputQuestion[],
  drafts: RequestUserInputDrafts
): {
  text: string | null;
  missingQuestionIds: string[];
} {
  const missingQuestionIds: string[] = [];
  const answers = new Map<string, RequestUserInputAnswerValue>();

  for (const question of questions) {
    const answer = buildRequestUserInputAnswerValue(question, drafts[question.id]);
    if (answer == null) {
      missingQuestionIds.push(question.id);
      continue;
    }
    answers.set(question.id, answer);
  }

  if (questions.length === 0) {
    return {
      text: null,
      missingQuestionIds,
    };
  }

  if (questions.length === 1) {
    const single = answers.get(questions[0].id);
    return {
      text: serializeSingleQuestionSubmission(single),
      missingQuestionIds,
    };
  }

  const payload = Object.fromEntries(
    questions.flatMap((question) => {
      const answer = answers.get(question.id);
      return answer == null ? [] : [[question.id, answer]];
    })
  );

  return {
    text: Object.keys(payload).length > 0 ? JSON.stringify(payload, null, 2) : null,
    missingQuestionIds,
  };
}

function serializeSingleQuestionSubmission(
  answer: RequestUserInputAnswerValue | undefined
): string | null {
  if (answer == null) {
    return null;
  }
  return typeof answer === "string" ? answer : JSON.stringify(answer);
}

function buildRequestUserInputAnswerValue(
  question: RequestUserInputQuestion,
  draft: RequestUserInputDraft | undefined
): RequestUserInputAnswerValue | null {
  const note = draft?.note?.trim() ?? "";
  const hasOptions = question.options != null && question.options.length > 0;

  if (!hasOptions) {
    return note || null;
  }

  const answers: string[] = [];
  const selectedOptionLabel = normalizeSelectedOptionLabel(question, draft?.selectedOptionLabel);
  if (selectedOptionLabel) {
    answers.push(selectedOptionLabel);
  }
  if (note) {
    answers.push(`user_note: ${note}`);
  }
  if (answers.length === 0) {
    return null;
  }
  return answers.length === 1 ? answers[0] : answers;
}

function normalizeSelectedOptionLabel(
  question: RequestUserInputQuestion,
  selectedOptionLabel: string | null | undefined
): string | null {
  const value = selectedOptionLabel?.trim();
  if (!value) {
    return null;
  }

  if (value === REQUEST_USER_INPUT_OTHER_OPTION_LABEL && question.isOther) {
    return value;
  }

  if (question.options?.some((option) => option.label === value)) {
    return value;
  }

  return null;
}

function parseRequestUserInputQuestion(value: unknown): RequestUserInputQuestion | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const id = asString(record.id);
  const question = asString(record.question);
  if (!id || !question) {
    return null;
  }

  return {
    id,
    header: asString(record.header) ?? "",
    question,
    isOther: Boolean(record.isOther),
    isSecret: Boolean(record.isSecret),
    options: parseRequestUserInputOptions(record.options),
  };
}

function parseRequestUserInputOptions(
  value: unknown
): RequestUserInputQuestionOption[] | null {
  if (!Array.isArray(value)) {
    return null;
  }
  const options = value
    .map((option) => {
      const record = asRecord(option);
      if (!record) {
        return null;
      }
      const label = asString(record.label);
      const description = asString(record.description);
      if (!label || !description) {
        return null;
      }
      return { label, description } satisfies RequestUserInputQuestionOption;
    })
    .filter((option): option is RequestUserInputQuestionOption => option !== null);
  return options.length > 0 ? options : null;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (value == null || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function asString(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}
