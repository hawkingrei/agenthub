import { describe, expect, it } from "vitest";
import {
  DEFAULT_TEAM_LEADER_PROMPT,
  DEFAULT_TEAM_WORKER_PROMPT,
} from "./member_helpers";

describe("team member prompt mirrors", () => {
  it("keeps leader prompt self-maintenance and time-trigger contract", () => {
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("profile_patch_proposal");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain('target="team"');
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain('target="run"');
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agent_time_trigger_set");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agent_time_trigger_list");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agent_time_trigger_cancel");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agent_loop");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("team-task-lifecycle");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("team_tasks");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("team_task_create");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("team_task_update");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("in_review");
    expect(DEFAULT_TEAM_LEADER_PROMPT).not.toContain("acp_permission_review_respond");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("human review card");
  });

  it("keeps worker prompt self-maintenance and time-trigger contract", () => {
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("profile_patch_proposal");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agent_time_trigger_set");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agent_time_trigger_list");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agent_time_trigger_cancel");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agent_loop");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain(".agenthubmemory/TODO.md");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("team-task-lifecycle");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("team_tasks");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("team_task_create");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("in_review");
    expect(DEFAULT_TEAM_WORKER_PROMPT).not.toContain("acp_permission_review_respond");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("human review card");
  });
});
