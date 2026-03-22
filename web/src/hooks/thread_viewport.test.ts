import { describe, expect, it } from "vitest";
import {
  deriveThreadJumpState,
  deriveThreadStickToBottom,
  nextThreadViewport,
  normalizeThreadAvgHeightEstimate,
  restoreThreadScrollTop,
} from "./thread_viewport";

describe("thread_viewport", () => {
  it("shows jump badge only when viewport is active, detached, and has pending items", () => {
    expect(
      deriveThreadJumpState({
        active: true,
        stickToBottom: false,
        pendingCount: 3,
      })
    ).toEqual({ showJump: true, showBadge: true });

    expect(
      deriveThreadJumpState({
        active: true,
        stickToBottom: false,
        pendingCount: 0,
      })
    ).toEqual({ showJump: true, showBadge: false });

    expect(
      deriveThreadJumpState({
        active: false,
        stickToBottom: false,
        pendingCount: 3,
      })
    ).toEqual({ showJump: false, showBadge: false });
  });

  it("drops stick-to-bottom when the user scrolls upward away from the tail", () => {
    expect(
      deriveThreadStickToBottom({
        scrollHeight: 2400,
        scrollTop: 1400,
        clientHeight: 600,
        wasStickToBottom: true,
        previousScrollTop: 1760,
      })
    ).toBe(false);

    expect(
      deriveThreadStickToBottom({
        scrollHeight: 2400,
        scrollTop: 1790,
        clientHeight: 600,
        wasStickToBottom: true,
        previousScrollTop: 1760,
      })
    ).toBe(true);
  });

  it("keeps viewport helpers stable for unchanged values and bounds restored scroll", () => {
    const viewport = { top: 120, height: 640 };
    expect(nextThreadViewport(viewport, 120, 640)).toBe(viewport);
    expect(nextThreadViewport(viewport, 180, 640)).toEqual({ top: 180, height: 640 });

    expect(normalizeThreadAvgHeightEstimate(40, 0, 0)).toBe(40);
    expect(normalizeThreadAvgHeightEstimate(40, 2400, 20)).toBe(120);
    expect(restoreThreadScrollTop(900, 1200, 500)).toBe(700);
  });
});
