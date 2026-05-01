// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  clearTeamCreateDraft,
  loadTeamCreateDraft,
  persistTeamCreateDraft,
} from "./create_draft_storage";
import { createInitialTeamCreateState, type TeamCreateState } from "./state";

class MemoryStorage {
  private data = new Map<string, string>();

  getItem(key: string): string | null {
    return this.data.has(key) ? this.data.get(key) ?? null : null;
  }

  setItem(key: string, value: string): void {
    this.data.set(key, value);
  }

  removeItem(key: string): void {
    this.data.delete(key);
  }
}

let originalStorage: unknown;

function buildState(
  patch: Partial<TeamCreateState> = {}
): TeamCreateState {
  return {
    ...createInitialTeamCreateState(),
    showCreateTeamModal: true,
    newTeamName: "alpha",
    coordinatorMemberId: "coordinator-1",
    workers: [
      {
        member_id: "worker-1",
        description: "",
        model: "codex",
        prompt: "worker prompt",
        skills: ["agenthub-actor-runtime", "team-worker-executor"],
        custom_skills: "",
      },
    ],
    ...patch,
  };
}

describe("team create draft storage", () => {
  beforeEach(() => {
    originalStorage = (globalThis as { localStorage?: unknown }).localStorage;
    (globalThis as { localStorage?: unknown }).localStorage = new MemoryStorage();
  });

  afterEach(() => {
    clearTeamCreateDraft();
    if (typeof originalStorage === "undefined") {
      delete (globalThis as { localStorage?: unknown }).localStorage;
      return;
    }
    (globalThis as { localStorage?: unknown }).localStorage = originalStorage;
  });

  it("persists and restores wizard draft", () => {
    persistTeamCreateDraft(buildState({ useSpecOverride: false, createTeamStage: 2 }));
    const restored = loadTeamCreateDraft("wizard");
    expect(restored.error).toBeNull();
    expect(restored.draft?.newTeamName).toBe("alpha");
    expect(restored.draft?.coordinatorMemberId).toBe("coordinator-1");
    expect(restored.draft?.createTeamStage).toBe(2);
    expect(restored.draft?.workers?.length).toBe(1);
  });

  it("returns null when entry mode does not match", () => {
    persistTeamCreateDraft(buildState({ useSpecOverride: false }));
    const restored = loadTeamCreateDraft("manual_spec");
    expect(restored.error).toBeNull();
    expect(restored.draft).toBeNull();
  });

  it("restores manual spec mode with override flag", () => {
    persistTeamCreateDraft(
      buildState({
        useSpecOverride: true,
        newTeamSpec: "{\"spec_version\":1}",
        createTeamStage: 3,
      })
    );
    const restored = loadTeamCreateDraft("manual_spec");
    expect(restored.error).toBeNull();
    expect(restored.draft?.useSpecOverride).toBe(true);
    expect(restored.draft?.newTeamSpec).toBe("{\"spec_version\":1}");
    expect(restored.draft?.createTeamStage).toBe(3);
  });

  it("clears persisted draft", () => {
    persistTeamCreateDraft(buildState());
    clearTeamCreateDraft();
    const restored = loadTeamCreateDraft("wizard");
    expect(restored.error).toBeNull();
    expect(restored.draft).toBeNull();
  });

  it("returns error and clears draft when payload is corrupted", () => {
    localStorage.setItem("agenthub_team_create_draft_v1", "{not-json");
    const restored = loadTeamCreateDraft("wizard");
    expect(restored.draft).toBeNull();
    expect(restored.error).toBe("Team create draft is corrupted and has been reset.");
    expect(localStorage.getItem("agenthub_team_create_draft_v1")).toBeNull();
  });

  it("skips persistence when modal is not open", () => {
    const result = persistTeamCreateDraft(buildState({ showCreateTeamModal: false }));
    expect(result).toBeNull();
    expect(localStorage.getItem("agenthub_team_create_draft_v1")).toBeNull();
  });

  it("returns save error when local storage write fails", () => {
    const originalSetItem = localStorage.setItem;
    localStorage.setItem = () => {
      throw new Error("quota exceeded");
    };
    try {
      const result = persistTeamCreateDraft(buildState());
      expect(result).toBe("Failed to save Team create draft locally.");
    } finally {
      localStorage.setItem = originalSetItem;
    }
  });

  it("returns error and clears draft when payload status is unknown", () => {
    localStorage.setItem(
      "agenthub_team_create_draft_v1",
      JSON.stringify({
        schema_version: 1,
        status: "done",
        entry_mode: "wizard",
        updated_at: Date.now(),
        draft: {},
      })
    );
    const restored = loadTeamCreateDraft("wizard");
    expect(restored.draft).toBeNull();
    expect(restored.error).toBe("Team create draft has unknown status and has been ignored.");
    expect(localStorage.getItem("agenthub_team_create_draft_v1")).toBeNull();
  });

  it("returns error and clears draft when payload draft is missing", () => {
    localStorage.setItem(
      "agenthub_team_create_draft_v1",
      JSON.stringify({
        schema_version: 1,
        status: "creating",
        entry_mode: "wizard",
        updated_at: Date.now(),
      })
    );
    const restored = loadTeamCreateDraft("wizard");
    expect(restored.draft).toBeNull();
    expect(restored.error).toBe("Team create draft is incomplete and has been ignored.");
    expect(localStorage.getItem("agenthub_team_create_draft_v1")).toBeNull();
  });

  it("ignores unknown schema versions without reporting an error", () => {
    localStorage.setItem(
      "agenthub_team_create_draft_v1",
      JSON.stringify({
        schema_version: 99,
        status: "creating",
        entry_mode: "wizard",
        updated_at: Date.now(),
        draft: { newTeamName: "alpha" },
      })
    );
    const restored = loadTeamCreateDraft("wizard");
    expect(restored.error).toBeNull();
    expect(restored.draft).toBeNull();
  });

  it("normalizes malformed draft fields and falls back to defaults", () => {
    const initial = createInitialTeamCreateState();
    localStorage.setItem(
      "agenthub_team_create_draft_v1",
      JSON.stringify({
        schema_version: 1,
        status: "creating",
        entry_mode: "wizard",
        updated_at: Date.now(),
        draft: {
          newTeamName: "beta",
          newTeamDescription: "desc",
          newTeamSpec: "",
          createTeamStage: 2.9,
          coordinatorMemberId: "coordinator-2",
          coordinatorModel: "",
          coordinatorPrompt: "",
          coordinatorSkills: "invalid",
          coordinatorCustomSkills: "custom",
          workers: [{ member_id: "worker-2", skills: ["team-worker-executor", ""] }, null],
          teamForgeAgentIds: ["coordinator-2", " ", "worker-2"],
        },
      })
    );
    const restored = loadTeamCreateDraft("wizard");
    expect(restored.error).toBeNull();
    expect(restored.draft?.newTeamName).toBe("beta");
    expect(restored.draft?.createTeamStage).toBe(2);
    expect(restored.draft?.coordinatorPrompt).toBe(initial.coordinatorPrompt);
    expect(restored.draft?.coordinatorSkills).toEqual(initial.coordinatorSkills);
    expect(restored.draft?.newTeamSpec).toBe(initial.newTeamSpec);
    expect(restored.draft?.workers).toEqual([
      {
        member_id: "worker-2",
        description: "",
        model: "",
        prompt: "",
        skills: ["team-worker-executor"],
        custom_skills: "",
      },
    ]);
    expect(restored.draft?.teamForgeAgentIds).toEqual(["coordinator-2", "worker-2"]);
  });

  it("loads legacy leader draft fields into coordinator state", () => {
    localStorage.setItem(
      "agenthub_team_create_draft_v1",
      JSON.stringify({
        schema_version: 1,
        status: "creating",
        entry_mode: "wizard",
        updated_at: Date.now(),
        draft: {
          newTeamName: "legacy-team",
          leaderMemberId: "legacy-coordinator",
          leaderModel: "gpt-legacy",
          leaderPrompt: "legacy prompt",
          leaderSkills: ["team-coordinator-orchestrator", ""],
          leaderCustomSkills: "legacy custom",
          workers: [],
          teamForgeAgentIds: ["legacy-coordinator"],
        },
      })
    );

    const restored = loadTeamCreateDraft("wizard");
    expect(restored.error).toBeNull();
    expect(restored.draft?.coordinatorMemberId).toBe("legacy-coordinator");
    expect(restored.draft?.coordinatorModel).toBe("gpt-legacy");
    expect(restored.draft?.coordinatorPrompt).toBe("legacy prompt");
    expect(restored.draft?.coordinatorSkills).toEqual(["team-coordinator-orchestrator"]);
    expect(restored.draft?.coordinatorCustomSkills).toBe("legacy custom");
    expect(restored.draft?.teamForgeAgentIds).toEqual(["legacy-coordinator"]);
  });
});
