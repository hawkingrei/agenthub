import { describe, expect, it } from "vitest";
import {
  CREATE_TEAM_STAGE_TITLES,
  DEFAULT_TEAM_CONTROL_STATE,
  DEFAULT_TEAM_MAILBOX_STATE,
  DEFAULT_TEAM_UI_STATE,
  MAILBOX_TEMPLATE_OPTIONS,
  TEAM_TAB_ITEMS,
  TEAM_RUN_STATUS_FILTER_OPTIONS,
  tabRequiresActiveRun,
  createInitialTeamCreateState,
  reduceTeamControlState,
  reduceTeamCreateState,
  reduceTeamMailboxState,
  reduceTeamUiState,
  resolveUpdater,
  type TeamControlAction,
  type TeamCreateAction,
  type TeamMailboxAction,
  type TeamUiAction,
} from "./state";

describe("team state reducers", () => {
  it("reduces TeamUiState actions and preserves state on unknown action", () => {
    const nextTab = reduceTeamUiState(DEFAULT_TEAM_UI_STATE, {
      type: "set_tab",
      tab: "mailbox",
    });
    expect(nextTab.tab).toBe("mailbox");

    const nextRunsTab = reduceTeamUiState(DEFAULT_TEAM_UI_STATE, {
      type: "set_tab",
      tab: "runs",
    });
    expect(nextRunsTab.tab).toBe("runs");

    const nextLookup = reduceTeamUiState(DEFAULT_TEAM_UI_STATE, {
      type: "set_run_lookup_id",
      runLookupId: "run-1",
    });
    expect(nextLookup.runLookupId).toBe("run-1");

    const nextAutoRefresh = reduceTeamUiState(DEFAULT_TEAM_UI_STATE, {
      type: "set_events_auto_refresh",
      eventsAutoRefresh: false,
    });
    expect(nextAutoRefresh.eventsAutoRefresh).toBe(false);

    const untouched = reduceTeamUiState(
      DEFAULT_TEAM_UI_STATE,
      { type: "unknown" } as unknown as TeamUiAction
    );
    expect(untouched).toBe(DEFAULT_TEAM_UI_STATE);
  });

  it("reduces TeamControlState patch and preserves state on unknown action", () => {
    const patched = reduceTeamControlState(DEFAULT_TEAM_CONTROL_STATE, {
      type: "patch",
      patch: {
        runContextId: "ctx-1",
        stepAction: "resume",
      },
    });
    expect(patched.runContextId).toBe("ctx-1");
    expect(patched.stepAction).toBe("resume");
    expect(patched.runInput).toBe(DEFAULT_TEAM_CONTROL_STATE.runInput);

    const untouched = reduceTeamControlState(
      DEFAULT_TEAM_CONTROL_STATE,
      { type: "unknown" } as unknown as TeamControlAction
    );
    expect(untouched).toBe(DEFAULT_TEAM_CONTROL_STATE);
  });

  it("reduces TeamMailboxState patch and mark/reset chat seen actions", () => {
    const patched = reduceTeamMailboxState(DEFAULT_TEAM_MAILBOX_STATE, {
      type: "patch",
      patch: {
        msgFromActorId: "leader",
        msgToActorId: "worker",
      },
    });
    expect(patched.msgFromActorId).toBe("leader");
    expect(patched.msgToActorId).toBe("worker");

    const noKey = reduceTeamMailboxState(DEFAULT_TEAM_MAILBOX_STATE, {
      type: "mark_conversation_seen",
      key: "",
      messageId: 10,
    });
    expect(noKey).toBe(DEFAULT_TEAM_MAILBOX_STATE);

    const marked = reduceTeamMailboxState(DEFAULT_TEAM_MAILBOX_STATE, {
      type: "mark_conversation_seen",
      key: "leader::worker",
      messageId: 10,
    });
    expect(marked.chatSeenByConversation["leader::worker"]).toBe(10);

    const stale = reduceTeamMailboxState(marked, {
      type: "mark_conversation_seen",
      key: "leader::worker",
      messageId: 9,
    });
    expect(stale).toBe(marked);

    const resetNoop = reduceTeamMailboxState(DEFAULT_TEAM_MAILBOX_STATE, {
      type: "reset_chat_seen",
    });
    expect(resetNoop).toBe(DEFAULT_TEAM_MAILBOX_STATE);

    const reset = reduceTeamMailboxState(marked, { type: "reset_chat_seen" });
    expect(reset.chatSeenByConversation).toEqual({});

    const untouched = reduceTeamMailboxState(
      DEFAULT_TEAM_MAILBOX_STATE,
      { type: "unknown" } as unknown as TeamMailboxAction
    );
    expect(untouched).toBe(DEFAULT_TEAM_MAILBOX_STATE);
  });

  it("reduces TeamCreateState patch and preserves state on unknown action", () => {
    const initial = createInitialTeamCreateState();
    const patched = reduceTeamCreateState(initial, {
      type: "patch",
      patch: {
        newTeamName: "Quant Team",
        createTeamStage: 2,
      },
    });
    expect(patched.newTeamName).toBe("Quant Team");
    expect(patched.createTeamStage).toBe(2);

    const untouched = reduceTeamCreateState(
      initial,
      { type: "unknown" } as unknown as TeamCreateAction
    );
    expect(untouched).toBe(initial);
  });

  it("resolves updater for both next value and updater function", () => {
    expect(resolveUpdater(1, 3)).toBe(3);
    expect(resolveUpdater(1, (prev) => prev + 4)).toBe(5);
  });
});

describe("team state defaults and constants", () => {
  it("builds initial team create state with expected defaults", () => {
    const initial = createInitialTeamCreateState();
    expect(initial.showCreateTeamModal).toBe(false);
    expect(initial.createTeamStage).toBe(0);
    expect(initial.newTeamSpec).toBe("{}");
    expect(initial.forgeAgentPresetId).toBe("codex");
    expect(initial.forgeAgentWorktreeMode).toBe("use_existing");
    expect(initial.forgeAgentCodeMode).toBe(true);
    expect(initial.leaderPrompt).toBe("");
    expect(initial.leaderSkills).toEqual(
      expect.arrayContaining(["agenthub-actor-runtime", "team-leader-orchestrator"])
    );
  });

  it("exposes stable option constants for UI controls", () => {
    expect(TEAM_TAB_ITEMS.map((tab) => tab.value)).toEqual([
      "conversation",
      "tasks",
      "runs",
      "agent_acp",
      "overview",
      "events",
      "steps",
      "mailbox",
      "member_console",
      "debug",
    ]);
    expect(tabRequiresActiveRun("runs")).toBe(false);
    expect(tabRequiresActiveRun("conversation")).toBe(false);
    expect(tabRequiresActiveRun("debug")).toBe(false);
    expect(tabRequiresActiveRun("events")).toBe(true);

    expect(CREATE_TEAM_STAGE_TITLES).toEqual([
      "Mission Brief",
      "Leader Forge",
      "Recruit Workers",
      "Launch Team",
    ]);
    expect(MAILBOX_TEMPLATE_OPTIONS.some((option) => option.value === "worker_blocked")).toBe(
      true
    );
    expect(
      TEAM_RUN_STATUS_FILTER_OPTIONS.map((option) => option.value)
    ).toEqual([
      "all",
      "submitted",
      "working",
      "input_required",
      "completed",
      "failed",
      "canceled",
    ]);
  });
});
