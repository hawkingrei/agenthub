import React from "react";
import {
  ACP_JUMP_BOTTOM_BUTTON_CLASS,
  INPUT_DOCK_HISTORY_BUTTON_CLASS,
  INPUT_DOCK_HISTORY_ITEM_CLASS,
  INPUT_DOCK_HISTORY_MENU_CLASS,
  INPUT_DOCK_INTERRUPT_BUTTON_CLASS,
  INPUT_DOCK_ROOT_CLASS,
  INPUT_DOCK_SEND_BUTTON_CLASS,
  INPUT_DOCK_TEXTAREA_CLASS,
} from "../ui/tailwind_classes";

type InputDockProps = {
  input: string;
  historyCommands: string[];
  showInterrupt: boolean;
  canInterrupt: boolean;
  sendDisabled?: boolean;
  onInputChange: (value: string) => void;
  onSendInput: () => void;
  onInterrupt: () => void;
  onNavigateHistory: (direction: "up" | "down") => void;
  onSelectHistoryCommand: (value: string) => void;
  onJumpToBottom: () => void;
  showConversationJump: boolean;
  isComposingRef: React.MutableRefObject<boolean>;
};

type VerticalRect = {
  top: number;
  bottom: number;
};

export type InputHistoryNavigationContext = {
  key: string;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  value: string;
  selectionStart: number | null;
  selectionEnd: number | null;
  isComposing: boolean;
};

export type InputDockKeyAction =
  | { type: "none" }
  | { type: "close_history" }
  | { type: "send" }
  | { type: "navigate_history"; direction: "up" | "down" };

type InputDockOutsideCloseDocument = Pick<
  Document,
  "addEventListener" | "removeEventListener"
>;

export function isImeComposing(
  currentRefState: boolean,
  nativeIsComposing: boolean,
  nativeKeyCode?: number
): boolean {
  return currentRefState || nativeIsComposing || nativeKeyCode === 229;
}

export function shouldCloseHistoryFromPointerTarget(
  target: EventTarget | null,
  container: { contains(node: Node): boolean } | null
): boolean {
  if (!container) return false;
  if (typeof Node === "undefined") return false;
  if (!(target instanceof Node)) return false;
  return !container.contains(target);
}

export function bindHistoryOutsideClose(
  doc: InputDockOutsideCloseDocument,
  container: { contains(node: Node): boolean } | null,
  onClose: () => void
): () => void {
  const handlePointerDown = (event: Event) => {
    if (!shouldCloseHistoryFromPointerTarget(event.target, container)) return;
    onClose();
  };
  doc.addEventListener("mousedown", handlePointerDown);
  doc.addEventListener("touchstart", handlePointerDown);
  return () => {
    doc.removeEventListener("mousedown", handlePointerDown);
    doc.removeEventListener("touchstart", handlePointerDown);
  };
}

export function deriveInputHistoryNavigation(
  ctx: InputHistoryNavigationContext
): "up" | "down" | null {
  if (ctx.isComposing) return null;
  if (ctx.key !== "ArrowUp" && ctx.key !== "ArrowDown") return null;
  if (ctx.shiftKey || ctx.altKey || ctx.metaKey || ctx.ctrlKey) return null;

  const value = ctx.value;
  const hasNewline = value.includes("\n");
  const selectionStart = ctx.selectionStart ?? 0;
  const selectionEnd = ctx.selectionEnd ?? selectionStart;
  const atStart = selectionStart === 0 && selectionEnd === 0;
  const atEnd = selectionStart === value.length && selectionEnd === value.length;

  if (ctx.key === "ArrowUp" && (atStart || !hasNewline)) return "up";
  if (ctx.key === "ArrowDown" && (atEnd || !hasNewline)) return "down";
  return null;
}

export type InputDockKeyActionContext = {
  key: string;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  sendOnEnter?: boolean;
  showHistory: boolean;
  composing: boolean;
  value: string;
  selectionStart: number | null;
  selectionEnd: number | null;
};

const MOBILE_INPUT_BREAKPOINT = 720;
const INPUT_VIEWPORT_SAFE_MARGIN = 12;

type RuntimeWindowLike = Pick<Window, "innerWidth" | "addEventListener" | "removeEventListener">;

function resolveRuntimeWindow(): RuntimeWindowLike | null {
  if (typeof window === "undefined") return null;
  return window;
}

export function isMobileInputViewport(width: number): boolean {
  return width <= MOBILE_INPUT_BREAKPOINT;
}

export function deriveInputPlaceholder(isMobileViewport: boolean): string {
  if (isMobileViewport) {
    return "Type a message (tap Send; Enter = newline)";
  }
  return "Send input (Enter to send, Shift+Enter for newline)";
}

export function isInputRectOutsideViewport(
  rect: VerticalRect,
  viewportTop: number,
  viewportHeight: number,
  safeMargin: number = INPUT_VIEWPORT_SAFE_MARGIN
): boolean {
  if (!Number.isFinite(rect.top) || !Number.isFinite(rect.bottom)) return false;
  if (!Number.isFinite(viewportTop) || !Number.isFinite(viewportHeight)) return false;
  if (viewportHeight <= 0) return false;
  const visibleTop = viewportTop + safeMargin;
  const visibleBottom = viewportTop + viewportHeight - safeMargin;
  return rect.top < visibleTop || rect.bottom > visibleBottom;
}

export function deriveInputViewportOffset(
  rect: VerticalRect,
  viewportTop: number,
  viewportHeight: number,
  safeMargin: number = INPUT_VIEWPORT_SAFE_MARGIN
): number {
  if (!Number.isFinite(rect.top) || !Number.isFinite(rect.bottom)) return 0;
  if (!Number.isFinite(viewportTop) || !Number.isFinite(viewportHeight)) return 0;
  if (viewportHeight <= 0) return 0;
  const visibleBottom = viewportTop + viewportHeight - safeMargin;
  if (!Number.isFinite(visibleBottom)) return 0;
  const overflow = rect.bottom - visibleBottom;
  if (overflow <= 0) return 0;
  return Math.ceil(overflow);
}

function useMobileInputViewport(): boolean {
  const runtimeWindow = resolveRuntimeWindow();
  const [isMobileViewport, setIsMobileViewport] = React.useState(() =>
    runtimeWindow ? isMobileInputViewport(runtimeWindow.innerWidth) : false
  );

  React.useEffect(() => {
    if (!runtimeWindow) return;
    const syncViewport = () => {
      setIsMobileViewport((previous) => {
        const next = isMobileInputViewport(runtimeWindow.innerWidth);
        return previous === next ? previous : next;
      });
    };
    syncViewport();
    runtimeWindow.addEventListener("resize", syncViewport);
    return () => {
      runtimeWindow.removeEventListener("resize", syncViewport);
    };
  }, [runtimeWindow]);

  return isMobileViewport;
}

export function deriveInputDockKeyAction(
  ctx: InputDockKeyActionContext
): InputDockKeyAction {
  if (ctx.key === "Escape" && ctx.showHistory) {
    return { type: "close_history" };
  }
  if (
    ctx.key === "Enter" &&
    !ctx.shiftKey &&
    !ctx.composing &&
    (ctx.sendOnEnter ?? true)
  ) {
    return { type: "send" };
  }
  const direction = deriveInputHistoryNavigation({
    key: ctx.key,
    shiftKey: ctx.shiftKey,
    altKey: ctx.altKey,
    metaKey: ctx.metaKey,
    ctrlKey: ctx.ctrlKey,
    value: ctx.value,
    selectionStart: ctx.selectionStart,
    selectionEnd: ctx.selectionEnd,
    isComposing: ctx.composing,
  });
  if (direction) {
    return { type: "navigate_history", direction };
  }
  return { type: "none" };
}

export function InputDock({
  input,
  historyCommands,
  showInterrupt,
  canInterrupt,
  sendDisabled = false,
  onInputChange,
  onSendInput,
  onInterrupt,
  onNavigateHistory,
  onSelectHistoryCommand,
  onJumpToBottom,
  showConversationJump,
  isComposingRef,
}: InputDockProps) {
  const textareaId = React.useId();
  const [showHistory, setShowHistory] = React.useState(false);
  const [inputFocused, setInputFocused] = React.useState(false);
  const historyContainerRef = React.useRef<HTMLDivElement | null>(null);
  const inputDockRef = React.useRef<HTMLDivElement | null>(null);
  const textareaRef = React.useRef<HTMLTextAreaElement | null>(null);
  const visibleHistory = historyCommands.slice(0, 12);
  const mobileInputViewport = useMobileInputViewport();
  const inputPlaceholder = deriveInputPlaceholder(mobileInputViewport);

  const ensureInputVisible = React.useCallback(() => {
    if (!mobileInputViewport) return;
    if (typeof window === "undefined") return;
    const textarea = textareaRef.current;
    if (!textarea) return;
    const viewport = window.visualViewport;
    const viewportTop = viewport?.offsetTop ?? 0;
    const viewportHeight = viewport?.height ?? window.innerHeight;
    const rect = textarea.getBoundingClientRect();
    const nextOffset = deriveInputViewportOffset(
      { top: rect.top, bottom: rect.bottom },
      viewportTop,
      viewportHeight
    );
    if (nextOffset > 0) {
      textarea.scrollIntoView({ block: "nearest", inline: "nearest" });
      inputDockRef.current?.scrollIntoView({ block: "end", inline: "nearest" });
      return;
    }
    if (
      !isInputRectOutsideViewport(
        { top: rect.top, bottom: rect.bottom },
        viewportTop,
        viewportHeight
      )
    ) {
      return;
    }
    inputDockRef.current?.scrollIntoView({ block: "end", inline: "nearest" });
  }, [mobileInputViewport]);

  React.useEffect(() => {
    if (!inputFocused || !mobileInputViewport) return;
    if (typeof window === "undefined") return;
    let rafId: number | null = null;
    const run = () => {
      if (typeof window.requestAnimationFrame === "function") {
        if (rafId != null && typeof window.cancelAnimationFrame === "function") {
          window.cancelAnimationFrame(rafId);
        }
        rafId = window.requestAnimationFrame(() => {
          rafId = null;
          ensureInputVisible();
        });
        return;
      }
      ensureInputVisible();
    };
    run();
    const viewport = window.visualViewport;
    viewport?.addEventListener("resize", run);
    viewport?.addEventListener("scroll", run);
    window.addEventListener("resize", run);
    return () => {
      if (
        rafId != null &&
        typeof window.cancelAnimationFrame === "function"
      ) {
        window.cancelAnimationFrame(rafId);
      }
      viewport?.removeEventListener("resize", run);
      viewport?.removeEventListener("scroll", run);
      window.removeEventListener("resize", run);
    };
  }, [ensureInputVisible, inputFocused, mobileInputViewport]);

  React.useEffect(() => {
    if (!showHistory) return;
    if (typeof document === "undefined") return;
    return bindHistoryOutsideClose(document, historyContainerRef.current, () => {
      setShowHistory(false);
    });
  }, [showHistory]);

  return (
    <div className="input-dock-shell flex flex-col gap-1.5" ref={inputDockRef}>
      {showConversationJump && (
        <button
          className={ACP_JUMP_BOTTOM_BUTTON_CLASS}
          onClick={onJumpToBottom}
          title="Jump to bottom"
          aria-label="Jump to bottom"
        >
          <i className="bi bi-chevron-down" aria-hidden="true" />
        </button>
      )}
      <div className={INPUT_DOCK_ROOT_CLASS}>
        <div
          className="input-row flex flex-wrap items-center justify-between gap-2"
          role="group"
          aria-label="Input actions"
          data-input-actions-row="true"
        >
          <div className="flex min-w-0 flex-wrap items-center gap-2">
          {showInterrupt && (
            <button
              className={INPUT_DOCK_INTERRUPT_BUTTON_CLASS}
              onClick={onInterrupt}
              disabled={!canInterrupt}
              title="Interrupt current run"
              aria-label="Interrupt current run"
            >
              Interrupt
            </button>
          )}
          {historyCommands.length > 0 && (
            <div className="input-history relative" ref={historyContainerRef}>
              <button
                className={INPUT_DOCK_HISTORY_BUTTON_CLASS}
                onClick={() => setShowHistory((prev) => !prev)}
                title="Show sent command history"
                aria-label="Show sent command history"
                aria-expanded={showHistory}
              >
                History
              </button>
              {showHistory && (
                <div className={INPUT_DOCK_HISTORY_MENU_CLASS}>
                  {visibleHistory.map((item, idx) => (
                    <button
                      key={`${idx}-${item}`}
                      className={INPUT_DOCK_HISTORY_ITEM_CLASS}
                      title={item}
                      onClick={() => {
                        onSelectHistoryCommand(item);
                        setShowHistory(false);
                      }}
                    >
                      {item}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
          </div>
        </div>
        <div className="input-editor-row flex items-end gap-2" data-input-editor-row="true">
          <textarea
            id={textareaId}
            name="acp_input"
            className={`${INPUT_DOCK_TEXTAREA_CLASS} flex-1`}
            ref={textareaRef}
            placeholder={inputPlaceholder}
            value={input}
            onFocus={() => {
              setInputFocused(true);
              ensureInputVisible();
            }}
            onBlur={() => {
              setInputFocused(false);
            }}
            onChange={(e) => {
              setShowHistory(false);
              onInputChange(e.target.value);
              ensureInputVisible();
            }}
            onCompositionStart={() => {
              isComposingRef.current = true;
            }}
            onCompositionEnd={() => {
              isComposingRef.current = false;
            }}
            onKeyDown={(e) => {
              const nativeEvent = e.nativeEvent as KeyboardEvent;
              const composing = isImeComposing(
                isComposingRef.current,
                nativeEvent.isComposing === true,
                nativeEvent.keyCode
              );
              const target = e.currentTarget;
              const action = deriveInputDockKeyAction({
                key: e.key,
                shiftKey: e.shiftKey,
                altKey: e.altKey,
                metaKey: e.metaKey,
                ctrlKey: e.ctrlKey,
                sendOnEnter: !mobileInputViewport,
                showHistory,
                composing,
                value: target.value,
                selectionStart: target.selectionStart,
                selectionEnd: target.selectionEnd,
              });
              if (action.type === "close_history") {
                setShowHistory(false);
                return;
              }
              if (action.type === "send") {
                if (sendDisabled) {
                  return;
                }
                e.preventDefault();
                onSendInput();
                setShowHistory(false);
                return;
              }
              if (action.type === "navigate_history") {
                e.preventDefault();
                setShowHistory(false);
                onNavigateHistory(action.direction);
              }
            }}
            rows={2}
          />
          <button
            className={INPUT_DOCK_SEND_BUTTON_CLASS}
            onClick={onSendInput}
            disabled={sendDisabled}
            aria-label="Send input"
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}
