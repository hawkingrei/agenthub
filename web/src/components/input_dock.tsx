import React from "react";

type InputDockProps = {
  input: string;
  historyCommands: string[];
  showInterrupt: boolean;
  canInterrupt: boolean;
  onInputChange: (value: string) => void;
  onSendInput: () => void;
  onInterrupt: () => void;
  onNavigateHistory: (direction: "up" | "down") => void;
  onSelectHistoryCommand: (value: string) => void;
  onJumpToBottom: () => void;
  showConversationJump: boolean;
  isComposingRef: React.MutableRefObject<boolean>;
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
  onInputChange,
  onSendInput,
  onInterrupt,
  onNavigateHistory,
  onSelectHistoryCommand,
  onJumpToBottom,
  showConversationJump,
  isComposingRef,
}: InputDockProps) {
  const [showHistory, setShowHistory] = React.useState(false);
  const historyContainerRef = React.useRef<HTMLDivElement | null>(null);
  const visibleHistory = historyCommands.slice(0, 12);
  const mobileInputViewport = useMobileInputViewport();
  const inputPlaceholder = deriveInputPlaceholder(mobileInputViewport);

  React.useEffect(() => {
    if (!showHistory) return;
    if (typeof document === "undefined") return;
    return bindHistoryOutsideClose(document, historyContainerRef.current, () => {
      setShowHistory(false);
    });
  }, [showHistory]);

  return (
    <div className="input docked">
      <div className="input-row" role="group" aria-label="Input actions">
        {showInterrupt && (
          <button
            className="acp-interrupt-button input-interrupt-button"
            onClick={onInterrupt}
            disabled={!canInterrupt}
            title="Interrupt current run"
            aria-label="Interrupt current run"
          >
            Interrupt
          </button>
        )}
        {historyCommands.length > 0 && (
          <div className="input-history" ref={historyContainerRef}>
            <button
              className="history-toggle"
              onClick={() => setShowHistory((prev) => !prev)}
              title="Show sent command history"
              aria-label="Show sent command history"
              aria-expanded={showHistory}
            >
              History
            </button>
            {showHistory && (
              <div className="input-history-menu">
                {visibleHistory.map((item, idx) => (
                  <button
                    key={`${idx}-${item}`}
                    className="input-history-item"
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
      {showConversationJump && (
        <button
          className="jump-bottom"
          onClick={onJumpToBottom}
          title="Jump to bottom"
          aria-label="Jump to bottom"
        >
          <i className="bi bi-chevron-down" aria-hidden="true" />
        </button>
      )}
      <div className="input-editor-row">
        <textarea
          placeholder={inputPlaceholder}
          value={input}
          onChange={(e) => {
            setShowHistory(false);
            onInputChange(e.target.value);
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
          className="input-send-button"
          onClick={onSendInput}
          aria-label="Send input"
        >
          Send
        </button>
      </div>
    </div>
  );
}
