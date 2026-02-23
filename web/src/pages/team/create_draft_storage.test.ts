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
    leaderMemberId: "leader-1",
    workers: [
      {
        member_id: "worker-1",
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
    expect(restored.draft?.leaderMemberId).toBe("leader-1");
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
});
