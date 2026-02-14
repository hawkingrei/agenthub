import { describe, expect, it } from "vitest";
import {
  bindHistoryOutsideClose,
  deriveInputDockKeyAction,
  deriveInputHistoryNavigation,
  isImeComposing,
  shouldCloseHistoryFromPointerTarget,
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

describe("deriveInputDockKeyAction", () => {
  it("closes history on Escape when menu is visible", () => {
    expect(
      deriveInputDockKeyAction({
        key: "Escape",
        shiftKey: false,
        altKey: false,
        metaKey: false,
        ctrlKey: false,
        showHistory: true,
        composing: false,
        value: "",
        selectionStart: 0,
        selectionEnd: 0,
      })
    ).toEqual({ type: "close_history" });
  });

  it("sends on Enter when not composing", () => {
    expect(
      deriveInputDockKeyAction({
        key: "Enter",
        shiftKey: false,
        altKey: false,
        metaKey: false,
        ctrlKey: false,
        showHistory: false,
        composing: false,
        value: "hi",
        selectionStart: 2,
        selectionEnd: 2,
      })
    ).toEqual({ type: "send" });
  });

  it("returns history navigation action when arrow key recalls history", () => {
    expect(
      deriveInputDockKeyAction({
        key: "ArrowUp",
        shiftKey: false,
        altKey: false,
        metaKey: false,
        ctrlKey: false,
        showHistory: false,
        composing: false,
        value: "",
        selectionStart: 0,
        selectionEnd: 0,
      })
    ).toEqual({ type: "navigate_history", direction: "up" });
  });

  it("returns none when no input dock shortcut applies", () => {
    expect(
      deriveInputDockKeyAction({
        key: "a",
        shiftKey: false,
        altKey: false,
        metaKey: false,
        ctrlKey: false,
        showHistory: false,
        composing: false,
        value: "",
        selectionStart: 0,
        selectionEnd: 0,
      })
    ).toEqual({ type: "none" });
  });
});

describe("history outside close helpers", () => {
  it("returns false when Node is unavailable", () => {
    const originalNode = (globalThis as { Node?: unknown }).Node;
    // Simulate non-browser environment where Node is undefined.
    (globalThis as { Node?: unknown }).Node = undefined;
    try {
      expect(
        shouldCloseHistoryFromPointerTarget({}, {
          contains: () => false,
        } as unknown as { contains(node: Node): boolean })
      ).toBe(false);
    } finally {
      (globalThis as { Node?: unknown }).Node = originalNode;
    }
  });

  it("binds and unbinds pointer listeners and closes on outside click", () => {
    class FakeNode {}
    const originalNode = (globalThis as { Node?: unknown }).Node;
    (globalThis as { Node?: unknown }).Node = FakeNode;
    const inside = new FakeNode();
    const outside = new FakeNode();

    const listeners = new Map<string, (event: Event) => void>();
    const added: string[] = [];
    const removed: string[] = [];
    const fakeDoc = {
      addEventListener: (name: string, handler: (event: Event) => void) => {
        added.push(name);
        listeners.set(name, handler);
      },
      removeEventListener: (name: string) => {
        removed.push(name);
        listeners.delete(name);
      },
    };
    let closeCount = 0;
    const cleanup = bindHistoryOutsideClose(
      fakeDoc as unknown as Document,
      {
        contains: (node: Node) => node === (inside as unknown as Node),
      },
      () => {
        closeCount += 1;
      }
    );

    expect(added).toEqual(["mousedown", "touchstart"]);
    listeners.get("mousedown")?.({ target: inside } as unknown as Event);
    expect(closeCount).toBe(0);
    listeners.get("mousedown")?.({ target: outside } as unknown as Event);
    expect(closeCount).toBe(1);

    cleanup();
    expect(removed).toEqual(["mousedown", "touchstart"]);
    (globalThis as { Node?: unknown }).Node = originalNode;
  });
});
