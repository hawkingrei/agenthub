import React from "react";

type InputDockProps = {
  input: string;
  onInputChange: (value: string) => void;
  onSendInput: () => void;
  onJumpToBottom: () => void;
  showConversationJump: boolean;
  isComposingRef: React.MutableRefObject<boolean>;
};

export function InputDock({
  input,
  onInputChange,
  onSendInput,
  onJumpToBottom,
  showConversationJump,
  isComposingRef,
}: InputDockProps) {
  return (
    <div className="input docked">
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
          }
        }}
        rows={2}
      />
      <button onClick={onSendInput}>Send</button>
    </div>
  );
}
