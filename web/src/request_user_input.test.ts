import { describe, expect, it } from "vitest";
import {
  REQUEST_USER_INPUT_OTHER_OPTION_LABEL,
  buildRequestUserInputSubmissionText,
  countAnsweredRequestUserInputQuestions,
  createInitialRequestUserInputDrafts,
  parseRequestUserInputQuestions,
  parseRequestUserInputResponse,
  splitRequestUserInputAnswer,
  type RequestUserInputQuestion,
} from "./request_user_input";

const sampleQuestions: RequestUserInputQuestion[] = [
  {
    id: "q1",
    header: "Reasoning scope",
    question: "Which reasoning scope should I use?",
    isOther: false,
    isSecret: false,
    options: null,
  },
];

describe("request_user_input helpers", () => {
  it("parses request_user_input raw_input only for synthetic request tool calls", () => {
    const parsed = parseRequestUserInputQuestions("request-user-input:call-1", [
      {
        id: "q1",
        header: "Reasoning scope",
        question: "Which reasoning scope should I use?",
        isOther: false,
        isSecret: false,
      },
    ]);

    expect(parsed).toEqual(sampleQuestions);
    expect(parseRequestUserInputQuestions("shell:call-1", [{ id: "q1" }])).toBeNull();
  });

  it("serializes a single freeform answer as plain text", () => {
    const drafts = createInitialRequestUserInputDrafts(sampleQuestions);
    drafts.q1.note = "Plan only";

    expect(buildRequestUserInputSubmissionText(sampleQuestions, drafts)).toEqual({
      text: "Plan only",
      missingQuestionIds: [],
    });
  });

  it("serializes multi-question option answers as a JSON object", () => {
    const questions: RequestUserInputQuestion[] = [
      {
        id: "scope",
        header: "Scope",
        question: "Choose a scope.",
        isOther: false,
        isSecret: false,
        options: [
          {
            label: "Plan only",
            description: "Update only Plan mode.",
          },
        ],
      },
      {
        id: "notes",
        header: "Notes",
        question: "Add context.",
        isOther: true,
        isSecret: false,
        options: [
          {
            label: "Reuse current plan",
            description: "Keep the current plan structure.",
          },
        ],
      },
    ];
    const drafts = createInitialRequestUserInputDrafts(questions);
    drafts.scope.selectedOptionLabel = "Plan only";
    drafts.notes.selectedOptionLabel = REQUEST_USER_INPUT_OTHER_OPTION_LABEL;
    drafts.notes.note = "Need a narrower reasoning budget.";

    expect(buildRequestUserInputSubmissionText(questions, drafts)).toEqual({
      text: JSON.stringify(
        {
          scope: "Plan only",
          notes: [
            REQUEST_USER_INPUT_OTHER_OPTION_LABEL,
            "user_note: Need a narrower reasoning budget.",
          ],
        },
        null,
        2
      ),
      missingQuestionIds: [],
    });
  });

  it("parses structured request_user_input responses and splits notes from selections", () => {
    const response = parseRequestUserInputResponse({
      answers: {
        scope: {
          answers: ["Plan only"],
        },
        notes: {
          answers: [
            REQUEST_USER_INPUT_OTHER_OPTION_LABEL,
            "user_note: Need a narrower reasoning budget.",
          ],
        },
      },
    });

    expect(response).toEqual({
      answers: {
        scope: { answers: ["Plan only"] },
        notes: {
          answers: [
            REQUEST_USER_INPUT_OTHER_OPTION_LABEL,
            "user_note: Need a narrower reasoning budget.",
          ],
        },
      },
    });
    expect(splitRequestUserInputAnswer(response?.answers.notes)).toEqual({
      options: [REQUEST_USER_INPUT_OTHER_OPTION_LABEL],
      note: "Need a narrower reasoning budget.",
    });
  });

  it("counts answered questions from parsed responses", () => {
    const questions: RequestUserInputQuestion[] = [
      {
        id: "scope",
        header: "Scope",
        question: "Choose a scope.",
        isOther: false,
        isSecret: false,
        options: null,
      },
      {
        id: "notes",
        header: "Notes",
        question: "Add context.",
        isOther: false,
        isSecret: false,
        options: null,
      },
    ];

    expect(
      countAnsweredRequestUserInputQuestions(
        questions,
        parseRequestUserInputResponse({
          answers: {
            scope: {
              answers: ["Plan only"],
            },
          },
        })
      )
    ).toBe(1);
  });
});
