import { describe, expect, it } from "vitest";
import {
  EMPTY_TEAM_PROMPT_DEFAULTS,
  backfillEmptyWorkerDraftPrompts,
  buildDefaultWorkerDraft,
  createInitialTeamDraftState,
  resolveTeamPromptForRole,
} from "./member_helpers";

const TEST_PROMPT_DEFAULTS = {
  leader_prompt: "leader-default-prompt",
  worker_prompt: "worker-default-prompt",
};

describe("team member helper prompt defaults", () => {
  it("starts with empty prompt defaults until the API payload arrives", () => {
    const initial = createInitialTeamDraftState();
    expect(initial.leaderPrompt).toBe("");
    expect(EMPTY_TEAM_PROMPT_DEFAULTS).toEqual({
      leader_prompt: "",
      worker_prompt: "",
    });
  });

  it("uses runtime prompt defaults for fresh create drafts and worker drafts", () => {
    const initial = createInitialTeamDraftState(TEST_PROMPT_DEFAULTS);
    const worker = buildDefaultWorkerDraft("worker-1", TEST_PROMPT_DEFAULTS);

    expect(initial.leaderPrompt).toBe(TEST_PROMPT_DEFAULTS.leader_prompt);
    expect(worker.prompt).toBe(TEST_PROMPT_DEFAULTS.worker_prompt);
  });

  it("resolves prompt text by role", () => {
    expect(resolveTeamPromptForRole(TEST_PROMPT_DEFAULTS, "leader")).toBe(
      TEST_PROMPT_DEFAULTS.leader_prompt
    );
    expect(resolveTeamPromptForRole(TEST_PROMPT_DEFAULTS, "worker")).toBe(
      TEST_PROMPT_DEFAULTS.worker_prompt
    );
    expect(resolveTeamPromptForRole(TEST_PROMPT_DEFAULTS, "unknown")).toBe(
      TEST_PROMPT_DEFAULTS.worker_prompt
    );
  });

  it("backfills only empty worker prompts when defaults arrive", () => {
    const workers = [
      {
        member_id: "worker-1",
        description: "",
        model: "",
        prompt: "",
        skills: [],
        custom_skills: "",
      },
      {
        member_id: "worker-2",
        description: "",
        model: "",
        prompt: "   ",
        skills: [],
        custom_skills: "",
      },
      {
        member_id: "worker-3",
        description: "",
        model: "",
        prompt: "custom worker prompt",
        skills: [],
        custom_skills: "",
      },
    ];

    const nextWorkers = backfillEmptyWorkerDraftPrompts(workers, TEST_PROMPT_DEFAULTS);

    expect(nextWorkers[0]?.prompt).toBe(TEST_PROMPT_DEFAULTS.worker_prompt);
    expect(nextWorkers[1]?.prompt).toBe(TEST_PROMPT_DEFAULTS.worker_prompt);
    expect(nextWorkers[2]?.prompt).toBe("custom worker prompt");
  });
});
