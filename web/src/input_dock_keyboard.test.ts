import { describe, expect, it } from "vitest";
import {
  deriveInputHistoryNavigation,
  isImeComposing,
  type InputHistoryNavigationContext,
} from "./components/input_dock";

function navContext(
  override: Partial<InputHistoryNavigationContext> = {}
): InputHistoryNavigationContext {
  return {
    key: "ArrowUp",
    shiftKey: false,
    altKey: false,
    metaKey: false,
    ctrlKey: false,
    value: "echo hello",
    selectionStart: 0,
    selectionEnd: 0,
    isComposing: false,
    ...override,
  };
}

describe("isImeComposing", () => {
  it("returns true when composing ref is active", () => {
    expect(isImeComposing(true, false, undefined)).toBe(true);
  });

  it("returns true when native event is composing", () => {
    expect(isImeComposing(false, true, undefined)).toBe(true);
  });

  it("returns true for IME keyCode=229 fallback", () => {
    expect(isImeComposing(false, false, 229)).toBe(true);
  });

  it("returns false when no composing signals are active", () => {
    expect(isImeComposing(false, false, 13)).toBe(false);
  });
});

describe("deriveInputHistoryNavigation", () => {
  it("returns up for single-line ArrowUp", () => {
    expect(deriveInputHistoryNavigation(navContext())).toBe("up");
  });

  it("returns down for single-line ArrowDown", () => {
    expect(
      deriveInputHistoryNavigation(navContext({ key: "ArrowDown" }))
    ).toBe("down");
  });

  it("handles multiline only at textarea boundaries", () => {
    const value = "line1\nline2";
    expect(
      deriveInputHistoryNavigation(
        navContext({
          key: "ArrowUp",
          value,
          selectionStart: 2,
          selectionEnd: 2,
        })
      )
    ).toBeNull();
    expect(
      deriveInputHistoryNavigation(
        navContext({
          key: "ArrowUp",
          value,
          selectionStart: 0,
          selectionEnd: 0,
        })
      )
    ).toBe("up");
    expect(
      deriveInputHistoryNavigation(
        navContext({
          key: "ArrowDown",
          value,
          selectionStart: value.length,
          selectionEnd: value.length,
        })
      )
    ).toBe("down");
  });

  it("returns null when modifiers are pressed", () => {
    expect(deriveInputHistoryNavigation(navContext({ shiftKey: true }))).toBeNull();
    expect(deriveInputHistoryNavigation(navContext({ altKey: true }))).toBeNull();
    expect(deriveInputHistoryNavigation(navContext({ metaKey: true }))).toBeNull();
    expect(deriveInputHistoryNavigation(navContext({ ctrlKey: true }))).toBeNull();
  });

  it("returns null while IME composing", () => {
    expect(
      deriveInputHistoryNavigation(navContext({ isComposing: true }))
    ).toBeNull();
  });
});
