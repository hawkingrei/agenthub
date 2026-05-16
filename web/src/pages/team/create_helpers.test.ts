import { describe, expect, it, vi } from "vitest";
import type { AgentRecord } from "../../api";
import type { WorkerDraft } from "./member_helpers";
import {
  appendTeamMemberToSpec,
  buildTeamMemberDraftFromSpec,
  buildTeamForgeCleanupWarning,
  buildEmptyTeamSpec,
  buildCoordinatorForgeDefaultWorkdir,
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
  teamSpecHasCoordinator,
  updateTeamMemberProfileInSpec,
} from "./create_helpers";
import type { TeamMemberProfileDraft } from "./create_helpers";

const TEST_PROMPT_DEFAULTS = {
  coordinator_prompt: "coordinator-default-prompt",
  worker_prompt: "worker-default-prompt",
};

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
    args: ["actor"],
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

function buildProfileDraft(
  overrides: Partial<TeamMemberProfileDraft> & Pick<TeamMemberProfileDraft, "member_id" | "role">
): TeamMemberProfileDraft {
  return {
    description: "",
    model: "",
    prompt: "",
    skills: [],
    custom_skills: "",
    agent_loop_enabled: false,
    agent_loop_idle_seconds: "",
    agent_loop_prompt: "",
    codex_acp_default_mode: "full-access",
    ...overrides,
  };
}

describe("team create helpers", () => {
  it("builds default team spec from form fields with normalized members and steps", () => {
    const spec = buildTeamSpecFromForm(
      "coordinator-main",
      "gpt-5",
      "Lead the mission",
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
        buildForgeAgent({ id: "coordinator-main", workdir: "/tmp/coordinator-main" }),
        buildForgeAgent({
          id: "worker alpha",
          name: "worker-alpha",
          workdir: "/tmp/team-workers/worker-alpha",
          worktree_mode: "create_worktree",
          worktree_repo: "/tmp/repos/shiro",
          worktree_ref: "HEAD",
          code_mode: true,
        }),
      ],
      TEST_PROMPT_DEFAULTS
    ) as {
      spec_version: number;
      entrypoint: string;
      coordinator_member_id: string;
      members: Array<{
        member_id: string;
        role: string;
        description?: string;
        model?: string;
        prompt: string;
        runtime?: Record<string, unknown>;
        skills?: string[];
      }>;
      steps: Array<{
        step_key: string;
        member_id: string;
        depends_on: string[];
      }>;
    };

    expect(spec.spec_version).toBe(1);
    expect(spec.entrypoint).toBe("coordinator_plan");
    expect(spec.coordinator_member_id).toBe("coordinator-main");
    expect(spec.members.map((member) => member.member_id)).toEqual([
      "coordinator-main",
      "worker alpha",
    ]);
    expect(spec.members[0]?.skills).toBeUndefined();
    expect(spec.members[1]?.prompt).toBe(TEST_PROMPT_DEFAULTS.worker_prompt);
    expect(spec.members[1]?.description).toBe("query optimizer specialist");
    expect(spec.members[1]?.skills).toBeUndefined();
    expect(spec.members[0]?.runtime).toEqual({
      name: "coordinator-main-name",
      target_node_id: null,
      workdir: "/tmp/coordinator-main",
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: false,
      codex_acp_default_mode: "full-access",
      agent_loop_enabled: false,
      agent_loop_idle_seconds: null,
      agent_loop_prompt: null,
    });
    expect(spec.members[1]?.runtime).toEqual({
      name: "worker-alpha",
      target_node_id: null,
      workdir: "/tmp/team-workers/worker-alpha",
      worktree_mode: "create_worktree",
      worktree_repo: "/tmp/repos/shiro",
      worktree_ref: "HEAD",
      code_mode: true,
      codex_acp_default_mode: "full-access",
      agent_loop_enabled: false,
      agent_loop_idle_seconds: null,
      agent_loop_prompt: null,
    });
    expect(spec.steps).toEqual([
      {
        step_key: "coordinator_plan",
        member_id: "coordinator-main",
        depends_on: [],
      },
      {
        step_key: "worker_1_worker_alpha",
        member_id: "worker alpha",
        depends_on: ["coordinator_plan"],
      },
      {
        step_key: "coordinator_synthesize",
        member_id: "coordinator-main",
        depends_on: ["worker_1_worker_alpha"],
      },
    ]);
  });

  it("falls back to single planning step when no workers are provided", () => {
    const spec = buildTeamSpecFromForm(
      "coordinator-only",
      "",
      "",
      [],
      [buildForgeAgent({ id: "coordinator-only" })],
      TEST_PROMPT_DEFAULTS
    ) as {
      entrypoint: string;
      members: Array<{ member_id: string; prompt: string; runtime?: Record<string, unknown> }>;
      steps: Array<{ step_key: string; member_id: string; depends_on: string[] }>;
    };
    expect(spec.entrypoint).toBe("coordinator_plan");
    expect(spec.steps).toEqual([
      {
        step_key: "coordinator_plan",
        member_id: "coordinator-only",
        depends_on: [],
      },
    ]);
    expect(spec.members[0]?.member_id).toBe("coordinator-only");
    expect(spec.members[0]?.prompt).toBe(TEST_PROMPT_DEFAULTS.coordinator_prompt);
    expect(spec.members[0]?.runtime).toEqual({
      name: "coordinator-only-name",
      target_node_id: null,
      workdir: "/tmp/coordinator-only",
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: false,
      codex_acp_default_mode: "full-access",
      agent_loop_enabled: false,
      agent_loop_idle_seconds: null,
      agent_loop_prompt: null,
    });
  });

  it("builds an empty team spec with no configured members", () => {
    const spec = buildEmptyTeamSpec() as { spec_version: number; members: unknown[] };
    expect(spec).toEqual({
      spec_version: 1,
      members: [],
    });
    expect(teamSpecHasConfiguredMembers(spec)).toBe(false);
    expect(teamSpecHasCoordinator(spec)).toBe(false);
  });

  it("appends coordinator and worker profiles into a team spec with derived workflow", () => {
    const coordinatorAgent = buildForgeAgent({
      id: "coordinator-1",
      name: "coordinator-1-name",
      workdir: "/tmp/coordinator-1",
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
    const withCoordinator = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      buildProfileDraft({
        member_id: "coordinator-1",
        role: "coordinator",
        description: "Team architect",
        model: "codex",
        skills: ["team-deliberation-rules"],
      }),
      coordinatorAgent,
      TEST_PROMPT_DEFAULTS
    ) as {
      coordinator_member_id: string;
      entrypoint: string;
      members: Array<Record<string, unknown>>;
      steps: Array<{ step_key: string; member_id: string; depends_on: string[] }>;
    };
    expect(teamSpecHasConfiguredMembers(withCoordinator)).toBe(true);
    expect(teamSpecHasCoordinator(withCoordinator)).toBe(true);
    expect(withCoordinator.coordinator_member_id).toBe("coordinator-1");
    expect(withCoordinator.entrypoint).toBe("coordinator_plan");
    expect(withCoordinator.members).toHaveLength(1);
    expect(withCoordinator.members[0]?.prompt).toBe(TEST_PROMPT_DEFAULTS.coordinator_prompt);
    expect(withCoordinator.steps).toEqual([
      {
        step_key: "coordinator_plan",
        member_id: "coordinator-1",
        depends_on: [],
      },
    ]);

    const withWorker = appendTeamMemberToSpec(
      withCoordinator,
      buildProfileDraft({
        member_id: "worker-1",
        role: "worker",
        description: "Implementation agent",
        prompt: "Execute with evidence",
        skills: ["team-deliberation-rules"],
        custom_skills: "custom-worker-skill",
      }),
      workerAgent,
      TEST_PROMPT_DEFAULTS
    ) as {
      coordinator_member_id: string;
      members: Array<Record<string, unknown>>;
      steps: Array<{ step_key: string; member_id: string; depends_on: string[] }>;
    };
    expect(withWorker.coordinator_member_id).toBe("coordinator-1");
    expect(withWorker.members).toHaveLength(2);
    expect(withWorker.members[1]?.prompt).toBe("Execute with evidence");
    expect(withWorker.steps).toEqual([
      {
        step_key: "coordinator_plan",
        member_id: "coordinator-1",
        depends_on: [],
      },
      {
        step_key: "worker_1_worker_1",
        member_id: "worker-1",
        depends_on: ["coordinator_plan"],
      },
      {
        step_key: "coordinator_synthesize",
        member_id: "coordinator-1",
        depends_on: ["worker_1_worker_1"],
      },
    ]);
  });

  it("rejects worker-first and duplicate coordinator append operations", () => {
    expect(() =>
      appendTeamMemberToSpec(
        buildEmptyTeamSpec(),
        buildProfileDraft({
          member_id: "worker-1",
          role: "worker",
        }),
        buildForgeAgent({ id: "worker-1" }),
        TEST_PROMPT_DEFAULTS
      )
    ).toThrow("Create the first agent as coordinator before adding more agents");

    const withCoordinator = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      buildProfileDraft({
        member_id: "coordinator-1",
        role: "coordinator",
      }),
      buildForgeAgent({ id: "coordinator-1" }),
      TEST_PROMPT_DEFAULTS
    );
    expect(() =>
      appendTeamMemberToSpec(
        withCoordinator,
        buildProfileDraft({
          member_id: "coordinator-2",
          role: "coordinator",
        }),
        buildForgeAgent({ id: "coordinator-2" }),
        TEST_PROMPT_DEFAULTS
      )
    ).toThrow("Team already has a coordinator");
  });

  it("builds editable member draft from spec and preserves role defaults", () => {
    const spec = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      buildProfileDraft({
        member_id: "coordinator-1",
        role: "coordinator",
        description: "Team architect",
        model: "codex",
        skills: ["team-deliberation-rules"],
      }),
      buildForgeAgent({ id: "coordinator-1" })
    );
    const draft = buildTeamMemberDraftFromSpec(spec, "coordinator-1", undefined, TEST_PROMPT_DEFAULTS);
    expect(draft).toMatchObject({
      member_id: "coordinator-1",
      role: "coordinator",
      description: "Team architect",
      model: "codex",
      custom_skills: "",
    });
    expect(draft?.prompt).toBe(TEST_PROMPT_DEFAULTS.coordinator_prompt);
    expect(draft?.skills).toEqual(
      expect.arrayContaining(["agenthub-actor-runtime", "team-coordinator-orchestrator"])
    );
  });

  it("round-trips codex startup mode through team runtime hints", () => {
    const spec = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      buildProfileDraft({
        member_id: "worker-1",
        role: "coordinator",
        model: "codex",
        codex_acp_default_mode: "auto",
      }),
      buildForgeAgent({
        id: "worker-1",
        command: "agenthub-codex-acp",
        codex_acp_default_mode: "auto",
      })
    ) as { members: Array<Record<string, unknown>> };

    expect(spec.members[0]?.runtime).toEqual(
      expect.objectContaining({
        codex_acp_default_mode: "auto",
      })
    );

    const draft = buildTeamMemberDraftFromSpec(spec, "worker-1");
    expect(draft?.codex_acp_default_mode).toBe("auto");

    const updated = updateTeamMemberProfileInSpec(spec, {
      ...draft!,
      codex_acp_default_mode: "yolo",
    }) as { members: Array<Record<string, unknown>> };
    expect(updated.members[0]?.runtime).toEqual(
      expect.objectContaining({
        codex_acp_default_mode: "full-access",
      })
    );
  });

  it("drops out-of-range loop idle seconds when building a member draft from spec", () => {
    const spec = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      buildProfileDraft({
        member_id: "coordinator-1",
        role: "coordinator",
        description: "Team architect",
        model: "codex",
        skills: ["team-deliberation-rules"],
      }),
      buildForgeAgent({ id: "coordinator-1" })
    ) as { members: Array<Record<string, unknown>> };
    spec.members[0] = {
      ...spec.members[0],
      runtime: {
        ...(spec.members[0]?.runtime as Record<string, unknown>),
        agent_loop_enabled: true,
        agent_loop_idle_seconds: 5,
        agent_loop_prompt: "Resume review synthesis after silence.",
      },
    };

    const draft = buildTeamMemberDraftFromSpec(spec, "coordinator-1", undefined, TEST_PROMPT_DEFAULTS);
    expect(draft?.agent_loop_idle_seconds).toBe("");
  });

  it("updates existing team member profile fields without replacing runtime hints", () => {
    const original = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      buildProfileDraft({
        member_id: "coordinator-1",
        role: "coordinator",
        description: "Team architect",
        model: "codex",
        skills: ["team-deliberation-rules"],
      }),
      buildForgeAgent({ id: "coordinator-1", workdir: "/tmp/coordinator-1", code_mode: true })
    );
    const updated = updateTeamMemberProfileInSpec(original, {
      member_id: "coordinator-1",
      role: "coordinator",
      description: "Review owner",
      model: "gpt-5.4",
      prompt: "Coordinate review and final synthesis.",
      skills: ["team-deliberation-rules"],
      custom_skills: "custom-skill",
      agent_loop_enabled: true,
      agent_loop_idle_seconds: "900",
      agent_loop_prompt: "Resume review synthesis after silence.",
      codex_acp_default_mode: "full-access",
    }) as { members: Array<Record<string, unknown>> };
    expect(updated.members).toHaveLength(1);
    expect(updated.members[0]).toMatchObject({
      member_id: "coordinator-1",
      role: "coordinator",
      description: "Review owner",
      model: "gpt-5.4",
      prompt: "Coordinate review and final synthesis.",
    });
    expect(updated.members[0]?.skills).toBeUndefined();
    expect(updated.members[0]?.runtime).toEqual({
      name: "coordinator-1-name",
      workdir: "/tmp/coordinator-1",
      target_node_id: null,
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      codex_acp_default_mode: "full-access",
      agent_loop_enabled: true,
      agent_loop_idle_seconds: 900,
      agent_loop_prompt: "Resume review synthesis after silence.",
    });
  });

  it("treats partially numeric agent loop idle input as unset", () => {
    const original = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      buildProfileDraft({
        member_id: "coordinator-1",
        role: "coordinator",
        description: "Team architect",
        model: "codex",
        skills: ["team-deliberation-rules"],
      }),
      buildForgeAgent({ id: "coordinator-1", workdir: "/tmp/coordinator-1", code_mode: true })
    );
    const updated = updateTeamMemberProfileInSpec(original, {
      member_id: "coordinator-1",
      role: "coordinator",
      description: "Review owner",
      model: "gpt-5.4",
      prompt: "Coordinate review and final synthesis.",
      skills: ["team-deliberation-rules"],
      custom_skills: "",
      agent_loop_enabled: true,
      agent_loop_idle_seconds: "900abc",
      agent_loop_prompt: "Resume review synthesis after silence.",
      codex_acp_default_mode: "full-access",
    }) as { members: Array<Record<string, unknown>> };

    expect(updated.members[0]?.runtime).toEqual({
      name: "coordinator-1-name",
      workdir: "/tmp/coordinator-1",
      target_node_id: null,
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      codex_acp_default_mode: "full-access",
      agent_loop_enabled: true,
      agent_loop_idle_seconds: undefined,
      agent_loop_prompt: "Resume review synthesis after silence.",
    });
  });

  it("treats out-of-range agent loop idle input as unset", () => {
    const original = appendTeamMemberToSpec(
      buildEmptyTeamSpec(),
      buildProfileDraft({
        member_id: "coordinator-1",
        role: "coordinator",
        description: "Team architect",
        model: "codex",
        skills: ["team-deliberation-rules"],
      }),
      buildForgeAgent({ id: "coordinator-1", workdir: "/tmp/coordinator-1", code_mode: true })
    );
    const updated = updateTeamMemberProfileInSpec(original, {
      member_id: "coordinator-1",
      role: "coordinator",
      description: "Review owner",
      model: "gpt-5.4",
      prompt: "Coordinate review and final synthesis.",
      skills: ["team-deliberation-rules"],
      custom_skills: "",
      agent_loop_enabled: true,
      agent_loop_idle_seconds: "9",
      agent_loop_prompt: "Resume review synthesis after silence.",
      codex_acp_default_mode: "full-access",
    }) as { members: Array<Record<string, unknown>> };

    expect(updated.members[0]?.runtime).toEqual({
      name: "coordinator-1-name",
      workdir: "/tmp/coordinator-1",
      target_node_id: null,
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      codex_acp_default_mode: "full-access",
      agent_loop_enabled: true,
      agent_loop_idle_seconds: undefined,
      agent_loop_prompt: "Resume review synthesis after silence.",
    });
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
          { member_id: "coordinator-gemini" },
          { member_id: "worker-1" },
          { member_id: "worker-1" },
          { member_id: "  " },
          { role: "worker" },
          null,
        ],
      })
    ).toEqual(["coordinator-gemini", "worker-1"]);
    expect(collectTeamSpecMemberIds({ members: "invalid" })).toEqual([]);
    expect(collectTeamSpecMemberIds(null)).toEqual([]);
  });

  it("resolves unused forged agents by subtracting selected member ids", () => {
    const stale = resolveUnusedTeamForgeAgentIds(
      ["coordinator-codex", "coordinator-gemini", "worker-1", "worker-1", "", "  "],
      {
        members: [{ member_id: "coordinator-gemini" }, { member_id: "worker-1" }],
      }
    );
    expect(stale).toEqual(["coordinator-codex"]);
    expect(resolveUnusedTeamForgeAgentIds(["a", "a", "b"], { invalid: true })).toEqual([
      "a",
      "b",
    ]);
  });

  it("builds coordinator forge default workdir under .agenthub root", () => {
    expect(
      buildCoordinatorForgeDefaultWorkdir(
        "~/.agenthub/worktrees",
        "My Coordinator Agent",
        1_706_000_000_000
      )
    ).toBe("~/.agenthub/worktrees/my-coordinator-agent-lrq4coow");

    expect(buildCoordinatorForgeDefaultWorkdir("", "###", 10)).toBe(
      "~/.agenthub/worktrees/coordinator-a"
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
      ["coordinator-1", "worker-2", "coordinator-1", " ", ""],
      deleteAgent
    );
    expect(deleteAgent).toHaveBeenCalledTimes(2);
    expect(result.deletedForgeAgentIds).toEqual(["coordinator-1"]);
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
