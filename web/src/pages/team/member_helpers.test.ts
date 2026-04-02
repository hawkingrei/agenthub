import { describe, expect, it } from "vitest";
import {
  DEFAULT_TEAM_LEADER_PROMPT,
  DEFAULT_TEAM_WORKER_PROMPT,
} from "./member_helpers";

describe("team member prompt mirrors", () => {
  it("keeps leader prompt aligned with canonical actor CLI contracts", () => {
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("profile_patch_proposal");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain('target="team"');
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain('target="run"');
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agenthub actor time-trigger-set");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agenthub actor time-trigger-list");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agenthub actor time-trigger-cancel");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agent_loop");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("team-task-lifecycle");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agenthub actor team-members");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agenthub actor team-tasks");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agenthub actor team-task-create");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("agenthub actor team-task-update");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("in_review");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("Inspect inbox regularly");
    expect(DEFAULT_TEAM_LEADER_PROMPT).not.toContain("Pull inbox regularly");
    expect(DEFAULT_TEAM_LEADER_PROMPT).not.toContain("acp_permission_review_respond");
    expect(DEFAULT_TEAM_LEADER_PROMPT).toContain("human-review request");
  });

  it("keeps worker prompt aligned with canonical actor CLI contracts", () => {
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("profile_patch_proposal");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agenthub actor time-trigger-set");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agenthub actor time-trigger-list");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agenthub actor time-trigger-cancel");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agent_loop");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain(".agenthubmemory/TODO.md");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("team-task-lifecycle");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agenthub actor team-members");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agenthub actor team-tasks");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("agenthub actor team-task-create");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("in_review");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("Receive inbox work");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("Treat inbox inspection as read-only");
    expect(DEFAULT_TEAM_WORKER_PROMPT).not.toContain("Acknowledge messages after reading");
    expect(DEFAULT_TEAM_WORKER_PROMPT).not.toContain("acp_permission_review_respond");
    expect(DEFAULT_TEAM_WORKER_PROMPT).toContain("human-review request");
  });
});
