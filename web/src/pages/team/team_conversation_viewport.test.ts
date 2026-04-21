import { describe, expect, it } from "vitest";
import {
  DEFAULT_TEAM_CONVERSATION_PIN_TO_BOTTOM_MIN_ITEMS,
  DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE,
  deriveTeamThreadJumpState,
  deriveTeamThreadStickToBottom,
  shouldDefaultTeamConversationStickToBottom,
  windowTeamConversation,
} from "./team_conversation_viewport";

describe("team_conversation_viewport", () => {
  it("keeps the recent tail window only while pinned to the bottom", () => {
    const items = Array.from({ length: 18 }, (_, index) => index + 1);
    expect(
      windowTeamConversation(items, true, DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE)
    ).toEqual({
      items: items.slice(-DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE),
      offset: 8,
      total: 18,
    });

    expect(windowTeamConversation(items, false, DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE)).toEqual(
      {
        items,
        offset: 0,
        total: 18,
      }
    );
  });

  it("only defaults to bottom pinning once the conversation is large enough", () => {
    expect(
      shouldDefaultTeamConversationStickToBottom(
        DEFAULT_TEAM_CONVERSATION_PIN_TO_BOTTOM_MIN_ITEMS - 1
      )
    ).toBe(false);
    expect(
      shouldDefaultTeamConversationStickToBottom(
        DEFAULT_TEAM_CONVERSATION_PIN_TO_BOTTOM_MIN_ITEMS
      )
    ).toBe(true);
  });

  it("shows jump affordance only when the viewport is detached", () => {
    expect(
      deriveTeamThreadJumpState({
        active: true,
        stickToBottom: false,
        pendingCount: 2,
      })
    ).toEqual({ showJump: true, showBadge: true });

    expect(
      deriveTeamThreadJumpState({
        active: true,
        stickToBottom: false,
        pendingCount: 0,
      })
    ).toEqual({ showJump: true, showBadge: false });

    expect(
      deriveTeamThreadJumpState({
        active: true,
        stickToBottom: true,
        pendingCount: 2,
      })
    ).toEqual({ showJump: false, showBadge: false });
  });

  it("drops stick-to-bottom after a real upward user scroll", () => {
    expect(
      deriveTeamThreadStickToBottom({
        scrollHeight: 2400,
        scrollTop: 1400,
        clientHeight: 600,
        wasStickToBottom: true,
        previousScrollTop: 1760,
      })
    ).toBe(false);

    expect(
      deriveTeamThreadStickToBottom({
        scrollHeight: 2400,
        scrollTop: 1790,
        clientHeight: 600,
        wasStickToBottom: true,
        previousScrollTop: 1760,
      })
    ).toBe(true);
  });
});
