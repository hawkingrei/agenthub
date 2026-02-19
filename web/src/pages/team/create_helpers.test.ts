import { describe, expect, it } from "vitest";
import type { WorkerDraft } from "./member_helpers";
import {
  buildTeamSpecFromForm,
  clampCreateTeamStage,
  formatTeamForgeWorktreeError,
  parseErrorMessage,
  parseOptionalInteger,
  parseOptionalJson,
  parseRequiredJson,
  resolveTeamModelOptions,
} from "./create_helpers";

function buildWorker(overrides: Partial<WorkerDraft> = {}): WorkerDraft {
  return {
    member_id: "worker-alpha",
    model: "gpt-5",
    prompt: "Run optimization",
    skills: ["team-deliberation-rules"],
    custom_skills: "",
    ...overrides,
  };
}

describe("team create helpers", () => {
  it("builds default team spec from form fields with normalized members and steps", () => {
    const spec = buildTeamSpecFromForm(
      "leader-main",
      "gpt-5",
      "Lead the mission",
      ["team-deliberation-rules"],
      "custom-leader-skill",
      [
        buildWorker({
          member_id: "worker alpha",
          model: "gpt-5-mini",
          prompt: "",
          custom_skills: "custom-worker-skill",
        }),
        buildWorker({ member_id: "   " }),
      ]
    ) as {
      spec_version: number;
      entrypoint: string;
      leader_member_id: string;
      members: Array<{
        member_id: string;
        role: string;
        model?: string;
        prompt: string;
        skills: string[];
      }>;
      steps: Array<{
        step_key: string;
        member_id: string;
        depends_on: string[];
      }>;
    };

    expect(spec.spec_version).toBe(1);
    expect(spec.entrypoint).toBe("leader_plan");
    expect(spec.leader_member_id).toBe("leader-main");
    expect(spec.members.map((member) => member.member_id)).toEqual([
      "leader-main",
      "worker alpha",
    ]);
    expect(spec.members[0]?.skills).toEqual(
      expect.arrayContaining([
        "agenthub-actor-runtime",
        "team-leader-orchestrator",
        "custom-leader-skill",
      ])
    );
    expect(spec.members[1]?.prompt).toContain("You are a Worker in an AgentHub team");
    expect(spec.members[1]?.skills).toEqual(
      expect.arrayContaining([
        "agenthub-actor-runtime",
        "team-worker-executor",
        "custom-worker-skill",
      ])
    );
    expect(spec.steps).toEqual([
      {
        step_key: "leader_plan",
        member_id: "leader-main",
        depends_on: [],
      },
      {
        step_key: "worker_1_worker_alpha",
        member_id: "worker alpha",
        depends_on: ["leader_plan"],
      },
      {
        step_key: "leader_synthesize",
        member_id: "leader-main",
        depends_on: ["worker_1_worker_alpha"],
      },
    ]);
  });

  it("falls back to single planning step when no workers are provided", () => {
    const spec = buildTeamSpecFromForm("leader-only", "", "", [], "", []) as {
      entrypoint: string;
      steps: Array<{ step_key: string; member_id: string; depends_on: string[] }>;
    };
    expect(spec.entrypoint).toBe("leader_plan");
    expect(spec.steps).toEqual([
      {
        step_key: "leader_plan",
        member_id: "leader-only",
        depends_on: [],
      },
    ]);
  });

  it("parses error message from plain and JSON-formatted errors", () => {
    expect(parseErrorMessage(new Error("request failed"))).toBe("request failed");
    expect(parseErrorMessage(new Error("{\"error\":\"forbidden\"}"))).toBe("forbidden");
    expect(parseErrorMessage(new Error("{\"detail\":\"raw\"}"))).toBe("{\"detail\":\"raw\"}");
    expect(parseErrorMessage("oops")).toBe("oops");
  });

  it("formats known team forge worktree errors to actionable messages", () => {
    expect(formatTeamForgeWorktreeError(new Error("unexpected crash"))).toBeNull();
    expect(formatTeamForgeWorktreeError(new Error("workdir not allowed by safe paths"))).toBe(
      "Workdir not allowed. Add the path to Safe Paths before creating this agent."
    );
    expect(formatTeamForgeWorktreeError(new Error("worktree repo is required"))).toBe(
      "Worktree repo is required for the selected mode."
    );
    expect(formatTeamForgeWorktreeError(new Error("worktree does not exist"))).toBe(
      "Worktree does not exist. Use Create Worktree or choose an existing workdir."
    );
    expect(formatTeamForgeWorktreeError(new Error("workdir is not empty"))).toBe(
      "Workdir is not empty. Choose an empty directory for Create Worktree."
    );
    expect(formatTeamForgeWorktreeError(new Error("git worktree add failed: exit 128"))).toBe(
      "Git worktree add failed. git worktree add failed: exit 128"
    );
    expect(formatTeamForgeWorktreeError(new Error("worktree validation failed"))).toBe(
      "worktree validation failed"
    );
  });

  it("validates required and optional JSON payload parsing", () => {
    expect(parseRequiredJson("{\"a\":1}", "spec")).toEqual({ a: 1 });
    expect(() => parseRequiredJson("  ", "spec")).toThrow("spec is required");
    expect(() => parseRequiredJson("{bad", "spec")).toThrow("spec must be valid JSON");

    expect(parseOptionalJson("   ", "payload")).toBeUndefined();
    expect(parseOptionalJson("{\"ok\":true}", "payload")).toEqual({ ok: true });
    expect(() => parseOptionalJson("{bad", "payload")).toThrow("payload must be valid JSON");
  });

  it("validates optional non-negative integer parsing", () => {
    expect(parseOptionalInteger("  ", "limit")).toBeUndefined();
    expect(parseOptionalInteger("0", "limit")).toBe(0);
    expect(parseOptionalInteger("42", "limit")).toBe(42);
    expect(() => parseOptionalInteger("-1", "limit")).toThrow(
      "limit must be a non-negative integer"
    );
    expect(() => parseOptionalInteger("abc", "limit")).toThrow(
      "limit must be a non-negative integer"
    );
  });

  it("clamps create team stage index to valid enum range", () => {
    expect(clampCreateTeamStage(-10)).toBe(0);
    expect(clampCreateTeamStage(0)).toBe(0);
    expect(clampCreateTeamStage(2)).toBe(2);
    expect(clampCreateTeamStage(999)).toBe(3);
  });

  it("resolves model options and appends custom current model when needed", () => {
    const presetOptions = resolveTeamModelOptions("codex");
    expect(presetOptions[0]).toEqual({ value: "", label: "Use default model" });
    expect(presetOptions.some((option) => option.value === "codex")).toBe(true);
    expect(presetOptions.some((option) => option.label.includes("Custom"))).toBe(false);

    const customOptions = resolveTeamModelOptions("my-custom-model");
    expect(customOptions.at(-1)).toEqual({
      value: "my-custom-model",
      label: "Custom (my-custom-model)",
    });
  });
});
