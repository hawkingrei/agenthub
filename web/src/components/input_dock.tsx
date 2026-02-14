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

export function isImeComposing(
  currentRefState: boolean,
  nativeIsComposing: boolean,
  nativeKeyCode?: number
): boolean {
  return currentRefState || nativeIsComposing || nativeKeyCode === 229;
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

  React.useEffect(() => {
    if (!showHistory) return;
    if (typeof document === "undefined") return;
    const handlePointerDown = (event: MouseEvent | TouchEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (!historyContainerRef.current?.contains(target)) {
        setShowHistory(false);
      }
    };
    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("touchstart", handlePointerDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("touchstart", handlePointerDown);
    };
  }, [showHistory]);

  return (
    <div className="input docked">
      <div className="input-row">
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
      <textarea
        placeholder="Send input (Enter to send, Shift+Enter for newline)"
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
          if (e.key === "Escape" && showHistory) {
            setShowHistory(false);
            return;
          }
          const nativeEvent = e.nativeEvent as KeyboardEvent;
          const composing = isImeComposing(
            isComposingRef.current,
            nativeEvent.isComposing === true,
            nativeEvent.keyCode
          );
          if (e.key === "Enter" && !e.shiftKey && !composing) {
            e.preventDefault();
            onSendInput();
            setShowHistory(false);
            return;
          }
          const target = e.currentTarget;
          const direction = deriveInputHistoryNavigation({
            key: e.key,
            shiftKey: e.shiftKey,
            altKey: e.altKey,
            metaKey: e.metaKey,
            ctrlKey: e.ctrlKey,
            value: target.value,
            selectionStart: target.selectionStart,
            selectionEnd: target.selectionEnd,
            isComposing: composing,
          });
          if (direction) {
            e.preventDefault();
            setShowHistory(false);
            onNavigateHistory(direction);
          }
        }}
        rows={2}
      />
      <button onClick={onSendInput}>Send</button>
    </div>
  );
}
