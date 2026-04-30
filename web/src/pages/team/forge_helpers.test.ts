import { describe, expect, it } from "vitest";
import {
  buildTeamAgentNameToken,
  buildTeamMemberProfileDraft,
  resolveInitialTeamMemberRole,
  resolveTeamForgeDefaults,
  resolveTeamMemberRoleOptions,
  resolveTeamMemberRoleProfile,
} from "./forge_helpers";

const TEST_PROMPT_DEFAULTS = {
  leader_prompt: "leader-default-prompt",
  worker_prompt: "worker-default-prompt",
};

describe("team forge helpers", () => {
  it("builds role-specific default drafts", () => {
    const leaderDraft = buildTeamMemberProfileDraft("leader", undefined, TEST_PROMPT_DEFAULTS);
    const workerDraft = buildTeamMemberProfileDraft("worker", undefined, TEST_PROMPT_DEFAULTS);

    expect(leaderDraft.role).toBe("leader");
    expect(leaderDraft.model).toBe("codex");
    expect(leaderDraft.skills).toEqual([]);
    expect(leaderDraft.prompt).toBe(TEST_PROMPT_DEFAULTS.leader_prompt);
    expect(workerDraft.role).toBe("worker");
    expect(workerDraft.model).toBe("codex");
    expect(workerDraft.skills).toEqual([]);
    expect(workerDraft.prompt).toBe(TEST_PROMPT_DEFAULTS.worker_prompt);
  });

  it("normalizes agent name tokens and initial role", () => {
    expect(buildTeamAgentNameToken("  Alpha Desk / Team  ")).toBe("alpha-desk-team");
    expect(resolveInitialTeamMemberRole(false)).toBe("leader");
    expect(resolveInitialTeamMemberRole(true)).toBe("worker");
  });

  it("exposes role options with team constraints", () => {
    expect(resolveTeamMemberRoleOptions(false)).toEqual([
      {
        value: "leader",
        label: "Leader",
        description: "Own planning, review, and final synthesis.",
        disabled: false,
      },
      {
        value: "worker",
        label: "Worker",
        description: "Unlock after the first leader exists.",
        disabled: true,
      },
    ]);

    expect(resolveTeamMemberRoleOptions(true)).toEqual([
      {
        value: "leader",
        label: "Leader",
        description: "Already assigned for this team.",
        disabled: true,
      },
      {
        value: "worker",
        label: "Worker",
        description: "Deliver execution, evidence, and implementation.",
        disabled: false,
      },
    ]);
  });

  it("resolves distinct role profile copy", () => {
    expect(resolveTeamMemberRoleProfile("leader")).toEqual({
      profileLabel: "Leader Profile",
      intro: "Add the planning agent that owns delegation, review, and final synthesis.",
      focus: "Own planning, review, and final synthesis.",
      skillsHint: "Role skills and system instructions are injected automatically.",
      promptHint: "Describe what this leader should own for the team.",
    });
    expect(resolveTeamMemberRoleProfile("worker")).toEqual({
      profileLabel: "Worker Profile",
      intro: "Add the execution agent that implements scoped work and reports evidence.",
      focus: "Deliver implementation, validation, and execution evidence.",
      skillsHint: "Role skills and system instructions are injected automatically.",
      promptHint: "Describe what this worker should help with for the team.",
    });
  });

  it("resolves leader and worker forge defaults", () => {
    const leaderDefaults = resolveTeamForgeDefaults({
      teamName: "Alpha Desk",
      teamSpec: { members: [] },
      role: "leader",
      workerCount: 0,
      defaultWorktreeRoot: "~/.agenthub/worktrees",
      agentPresetId: "gemini",
      promptDefaults: TEST_PROMPT_DEFAULTS,
    });
    expect(leaderDefaults.agentName).toBe("alpha-desk-leader");
    expect(leaderDefaults.agentWorkdir).toMatch(
      /^~\/\.agenthub\/worktrees\/alpha-desk-leader-[a-z0-9]+$/
    );
    expect(leaderDefaults.worktreeMode).toBe("use_existing");
    expect(leaderDefaults.draft.role).toBe("leader");
    expect(leaderDefaults.draft.model).toBe("gemini");

    const workerDefaults = resolveTeamForgeDefaults({
      teamName: "Alpha Desk",
      teamSpec: {
        members: [
          {
            member_id: "alpha-desk-leader",
            role: "leader",
            runtime: {
              workdir: "/Users/weizhenwang/devel/opensource/agent/tidb",
              worktree_mode: "use_existing",
              worktree_repo: null,
            },
          },
        ],
      },
      role: "worker",
      workerCount: 2,
      defaultWorktreeRoot: "~/.agenthub/worktrees",
      agentPresetId: "kimi",
      promptDefaults: TEST_PROMPT_DEFAULTS,
    });
    expect(workerDefaults.agentName).toBe("alpha-desk-worker-3");
    expect(workerDefaults.agentWorkdir).toBe("/Users/weizhenwang/devel/opensource/agent/tidb");
    expect(workerDefaults.worktreeMode).toBe("use_existing");
    expect(workerDefaults.worktreeRepo).toBe("/Users/weizhenwang/devel/opensource/agent/tidb");
    expect(workerDefaults.draft.role).toBe("worker");
    expect(workerDefaults.draft.model).toBe("kimi");
  });

  it("does not use worktree_repo as the worker workdir default", () => {
    const workerDefaults = resolveTeamForgeDefaults({
      teamName: "Alpha Desk",
      teamSpec: {
        members: [
          {
            member_id: "worker-1",
            role: "worker",
            runtime: {
              workdir: "/Users/weizhenwang/devel/opensource/agent/shiro",
              worktree_mode: "create_worktree",
              worktree_repo: "/Users/weizhenwang/devel/opensource/agent/tidb",
            },
          },
        ],
      },
      role: "worker",
      workerCount: 0,
      defaultWorktreeRoot: "~/.agenthub/worktrees",
      promptDefaults: TEST_PROMPT_DEFAULTS,
    });

    expect(workerDefaults.agentWorkdir).toBe(
      "/Users/weizhenwang/devel/opensource/agent/shiro"
    );
    expect(workerDefaults.worktreeRepo).toBe(
      "/Users/weizhenwang/devel/opensource/agent/tidb"
    );
  });

  it("falls back to leader workdir when no runtime worktree repo exists", () => {
    const workerDefaults = resolveTeamForgeDefaults({
      teamName: "Alpha Desk",
      teamSpec: {
        members: [
          {
            member_id: "alpha-desk-leader",
            role: "leader",
            runtime: {
              workdir: "/Users/weizhenwang/devel/opensource/agent/tidb",
              worktree_mode: "use_existing",
              worktree_repo: null,
            },
          },
        ],
      },
      role: "worker",
      workerCount: 1,
      defaultWorktreeRoot: "~/.agenthub/worktrees",
      promptDefaults: TEST_PROMPT_DEFAULTS,
    });

    expect(workerDefaults.agentWorkdir).toBe(
      "/Users/weizhenwang/devel/opensource/agent/tidb"
    );
    expect(workerDefaults.worktreeRepo).toBe(
      "/Users/weizhenwang/devel/opensource/agent/tidb"
    );
  });
});
