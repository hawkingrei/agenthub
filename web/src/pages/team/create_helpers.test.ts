import { describe, expect, it, vi } from "vitest";
import type { AgentRecord } from "../../api";
import type { WorkerDraft } from "./member_helpers";
import {
  appendTeamMemberToSpec,
  buildTeamForgeCleanupWarning,
  buildEmptyTeamSpec,
  buildLeaderForgeDefaultWorkdir,
  buildTeamSpecFromForm,
  clampCreateTeamStage,
  cleanupUnusedTeamForgeAgents,
  collectTeamSpecMemberIds,
  formatTeamForgeWorktreeError,
  parseErrorMessage,
  parseOptionalInteger,
  parseOptionalJson,
  parseRequiredJson,
  resolveUnusedTeamForgeAgentIds,
  teamSpecHasConfiguredMembers,
  teamSpecHasLeader,
} from "./create_helpers";

function buildWorker(overrides: Partial<WorkerDraft> = {}): WorkerDraft {
  return {
    member_id: "worker-alpha",
    description: "",
    model: "gpt-5",
    prompt: "Run optimization",
    skills: ["team-deliberation-rules"],
    custom_skills: "",
    ...overrides,
  };
}

function buildForgeAgent(overrides: Partial<AgentRecord> = {}): AgentRecord {
  const id = overrides.id ?? "agent-default";
  return {
    id,
    name: `${id}-name`,
    workdir: `/tmp/${id}`,
    command: "agenthub",
    args: ["actor-mcp"],
    worktree_mode: "use_existing",
    worktree_repo: null,
    worktree_ref: null,
    code_mode: false,
    status: "created",
    created_at: 1,
    updated_at: 1,
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
          description: "query optimizer specialist",
          model: "gpt-5-mini",
          prompt: "",
          custom_skills: "custom-worker-skill",
        }),
        buildWorker({ member_id: "   " }),
      ],
      [
        buildForgeAgent({ id: "leader-main", workdir: "/tmp/leader-main" }),
        buildForgeAgent({
          id: "worker alpha",
          name: "worker-alpha",
          workdir: "/tmp/team-workers/worker-alpha",
          worktree_mode: "create_worktree",
          worktree_repo: "/tmp/repos/shiro",
          worktree_ref: "HEAD",
          code_mode: true,
        }),
      ]
    ) as {
      spec_version: number;
      entrypoint: string;
      leader_member_id: string;
      members: Array<{
        member_id: string;
        role: string;
        description?: string;
        model?: string;
        prompt: string;
        runtime?: Record<string, unknown>;
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
    expect(spec.members[1]?.description).toBe("query optimizer specialist");
    expect(spec.members[1]?.prompt).toContain("Work in your own git worktree only.");
    expect(spec.members[1]?.prompt).toContain("Create a random branch at start");
    expect(spec.members[1]?.skills).toEqual(
      expect.arrayContaining([
        "agenthub-actor-runtime",
        "team-worker-executor",
        "custom-worker-skill",
      ])
    );
    expect(spec.members[0]?.runtime).toEqual({
      name: "leader-main-name",
      workdir: "/tmp/leader-main",
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: false,
    });
    expect(spec.members[1]?.runtime).toEqual({
      name: "worker-alpha",
      workdir: "/tmp/team-workers/worker-alpha",
      worktree_mode: "create_worktree",
      worktree_repo: "/tmp/repos/shiro",
      worktree_ref: "HEAD",
      code_mode: true,
    });
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
    const spec = buildTeamSpecFromForm(
      "leader-only",
      "",
      "",
      [],
      "",
      [],
      [buildForgeAgent({ id: "leader-only" })]
    ) as {
      entrypoint: string;
      members: Array<{ member_id: string; prompt: string; runtime?: Record<string, unknown> }>;
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
    expect(spec.members[0]?.member_id).toBe("leader-only");
    expect(spec.members[0]?.prompt).toContain("Decision Complete");
    expect(spec.members[0]?.prompt).toContain("Explore Before Asking");
    expect(spec.members[0]?.prompt).toContain("Clearance checklist before delegation");
    expect(spec.members[0]?.prompt).toContain("spec.members[].member_id");
    expect(spec.members[0]?.prompt).toContain("Finalization by mode");
    expect(spec.members[0]?.runtime).toEqual({
      name: "leader-only-name",
      workdir: "/tmp/leader-only",
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: false,
    });
  });

  it("builds an empty team spec with no configured members", () => {
    const spec = buildEmptyTeamSpec() as { spec_version: number; members: unknown[] };
    expect(spec).toEqual({
      spec_version: 1,
      members: [],
    });
    expect(teamSpecHasConfiguredMembers(spec)).toBe(false);
    expect(teamSpecHasLeader(spec)).toBe(false);
  });

  it("appends leader and worker profiles into a team spec with derived workflow", () => {
    const leaderAgent = buildForgeAgent({
      id: "leader-1",
      name: "leader-1-name",
      workdir: "/tmp/leader-1",
      code_mode: true,
    });
    const workerAgent = buildForgeAgent({
      id: "worker-1",
      name: "worker-1-name",
      workdir: "/tmp/worker-1",
      worktree_mode: "create_worktree",
      worktree_repo: "/tmp/repos/agenthub",
      worktree_ref: "main",
      code_mode: true,
    });
    const withLeader = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      {
        member_id: "leader-1",
        role: "leader",
        description: "Team architect",
        model: "codex",
        prompt: "",
        skills: ["team-deliberation-rules"],
        custom_skills: "",
      },
      leaderAgent
    ) as {
      leader_member_id: string;
      entrypoint: string;
      members: Array<Record<string, unknown>>;
      steps: Array<{ step_key: string; member_id: string; depends_on: string[] }>;
    };
    expect(teamSpecHasConfiguredMembers(withLeader)).toBe(true);
    expect(teamSpecHasLeader(withLeader)).toBe(true);
    expect(withLeader.leader_member_id).toBe("leader-1");
    expect(withLeader.entrypoint).toBe("leader_plan");
    expect(withLeader.members).toHaveLength(1);
    expect(withLeader.steps).toEqual([
      {
        step_key: "leader_plan",
        member_id: "leader-1",
        depends_on: [],
      },
    ]);

    const withWorker = appendTeamMemberToSpec(
      withLeader,
      {
        member_id: "worker-1",
        role: "worker",
        description: "Implementation agent",
        model: "",
        prompt: "Execute with evidence",
        skills: ["team-deliberation-rules"],
        custom_skills: "custom-worker-skill",
      },
      workerAgent
    ) as {
      leader_member_id: string;
      members: Array<Record<string, unknown>>;
      steps: Array<{ step_key: string; member_id: string; depends_on: string[] }>;
    };
    expect(withWorker.leader_member_id).toBe("leader-1");
    expect(withWorker.members).toHaveLength(2);
    expect(withWorker.steps).toEqual([
      {
        step_key: "leader_plan",
        member_id: "leader-1",
        depends_on: [],
      },
      {
        step_key: "worker_1_worker_1",
        member_id: "worker-1",
        depends_on: ["leader_plan"],
      },
      {
        step_key: "leader_synthesize",
        member_id: "leader-1",
        depends_on: ["worker_1_worker_1"],
      },
    ]);
  });

  it("rejects worker-first and duplicate leader append operations", () => {
    expect(() =>
      appendTeamMemberToSpec(
        buildEmptyTeamSpec(),
        {
          member_id: "worker-1",
          role: "worker",
          description: "",
          model: "",
          prompt: "",
          skills: [],
          custom_skills: "",
        },
        buildForgeAgent({ id: "worker-1" })
      )
    ).toThrow("Create the first agent before adding more agents");

    const withLeader = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      {
        member_id: "leader-1",
        role: "leader",
        description: "",
        model: "",
        prompt: "",
        skills: [],
        custom_skills: "",
      },
      buildForgeAgent({ id: "leader-1" })
    );
    expect(() =>
      appendTeamMemberToSpec(
        withLeader,
        {
          member_id: "leader-2",
          role: "leader",
          description: "",
          model: "",
          prompt: "",
          skills: [],
          custom_skills: "",
        },
        buildForgeAgent({ id: "leader-2" })
      )
    ).toThrow("Team already has a leader");
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

    const parseSpy = vi.spyOn(JSON, "parse").mockImplementationOnce(() => {
      throw "broken parser";
    });
    expect(() => parseOptionalJson("{\"ok\":true}", "payload")).toThrow(
      "payload must be valid JSON (unknown parse error)"
    );
    parseSpy.mockRestore();
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

  it("collects unique member ids from team spec members", () => {
    expect(
      collectTeamSpecMemberIds({
        members: [
          { member_id: "leader-gemini" },
          { member_id: "worker-1" },
          { member_id: "worker-1" },
          { member_id: "  " },
          { role: "worker" },
          null,
        ],
      })
    ).toEqual(["leader-gemini", "worker-1"]);
    expect(collectTeamSpecMemberIds({ members: "invalid" })).toEqual([]);
    expect(collectTeamSpecMemberIds(null)).toEqual([]);
  });

  it("resolves unused forged agents by subtracting selected member ids", () => {
    const stale = resolveUnusedTeamForgeAgentIds(
      ["leader-codex", "leader-gemini", "worker-1", "worker-1", "", "  "],
      {
        members: [{ member_id: "leader-gemini" }, { member_id: "worker-1" }],
      }
    );
    expect(stale).toEqual(["leader-codex"]);
    expect(resolveUnusedTeamForgeAgentIds(["a", "a", "b"], { invalid: true })).toEqual([
      "a",
      "b",
    ]);
  });

  it("builds leader forge default workdir under .agenthub root", () => {
    expect(
      buildLeaderForgeDefaultWorkdir(
        "~/.agenthub/worktrees",
        "My Leader Agent",
        1_706_000_000_000
      )
    ).toBe("~/.agenthub/worktrees/my-leader-agent-lrq4coow");

    expect(buildLeaderForgeDefaultWorkdir("", "###", 10)).toBe(
      "~/.agenthub/worktrees/leader-a"
    );
  });

  it("cleans up stale forged agents and collects failures", async () => {
    const deleteAgent = vi.fn(async (_token: string, agentId: string) => {
      if (agentId === "worker-2") {
        throw new Error("permission denied");
      }
    });
    const result = await cleanupUnusedTeamForgeAgents(
      "token-1",
      ["leader-1", "worker-2", "leader-1", " ", ""],
      deleteAgent
    );
    expect(deleteAgent).toHaveBeenCalledTimes(2);
    expect(result.deletedForgeAgentIds).toEqual(["leader-1"]);
    expect(result.cleanupErrors).toEqual(["worker-2: permission denied"]);
  });

  it("returns empty cleanup result when no stale forged agents exist", async () => {
    const deleteAgent = vi.fn(async () => {});
    const result = await cleanupUnusedTeamForgeAgents("token-1", [], deleteAgent);
    expect(result).toEqual({ deletedForgeAgentIds: [], cleanupErrors: [] });
    expect(deleteAgent).not.toHaveBeenCalled();
  });

  it("builds cleanup warning only when failures exist", () => {
    expect(buildTeamForgeCleanupWarning([])).toBeNull();
    expect(buildTeamForgeCleanupWarning(["a: denied", "b: timeout"])).toBe(
      "Team created, but failed to clean up 2 unused forged agent(s): a: denied; b: timeout"
    );
  });
});
