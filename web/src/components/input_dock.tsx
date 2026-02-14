import React from "react";

type InputDockProps = {
  input: string;
  historyCommands: string[];
  onInputChange: (value: string) => void;
  onSendInput: () => void;
  onNavigateHistory: (direction: "up" | "down") => void;
  onSelectHistoryCommand: (value: string) => void;
  onJumpToBottom: () => void;
  showConversationJump: boolean;
  isComposingRef: React.MutableRefObject<boolean>;
};

export function InputDock({
  input,
  historyCommands,
  onInputChange,
  onSendInput,
  onNavigateHistory,
  onSelectHistoryCommand,
  onJumpToBottom,
  showConversationJump,
  isComposingRef,
}: InputDockProps) {
  const [showHistory, setShowHistory] = React.useState(false);
  const visibleHistory = historyCommands.slice(0, 12);
  return (
    <div className="input docked">
      <div className="input-row">
        {historyCommands.length > 0 && (
          <div className="input-history">
            <button
              className="history-toggle"
              onClick={() => setShowHistory((prev) => !prev)}
              title="Show sent command history"
              aria-label="Show sent command history"
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
        onChange={(e) => onInputChange(e.target.value)}
        onCompositionStart={() => {
          isComposingRef.current = true;
        }}
        onCompositionEnd={() => {
          isComposingRef.current = false;
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey && !isComposingRef.current) {
            e.preventDefault();
            onSendInput();
            setShowHistory(false);
            return;
          }
          if (
            (e.key === "ArrowUp" || e.key === "ArrowDown") &&
            !e.shiftKey &&
            !e.altKey &&
            !e.metaKey &&
            !e.ctrlKey &&
            !isComposingRef.current
          ) {
            const target = e.currentTarget;
            const value = target.value;
            const hasNewline = value.includes("\n");
            const selectionStart = target.selectionStart ?? 0;
            const selectionEnd = target.selectionEnd ?? selectionStart;
            const atStart = selectionStart === 0 && selectionEnd === 0;
            const atEnd =
              selectionStart === value.length && selectionEnd === value.length;
            const canHandleUp = e.key === "ArrowUp" && (atStart || !hasNewline);
            const canHandleDown = e.key === "ArrowDown" && (atEnd || !hasNewline);
            if (canHandleUp || canHandleDown) {
              e.preventDefault();
              onNavigateHistory(e.key === "ArrowUp" ? "up" : "down");
            }
          }
        }}
        rows={2}
      />
      <button onClick={onSendInput}>Send</button>
    </div>
  );
}
