import React from "react";
import { pushInputHistory } from "../../input_history";

type UseTeamMemberAcpInputArgs = {
  selectedMemberId: string;
  selectedSessionId: string | null;
  onSendInput?: (input: string, sessionId: string) => Promise<void> | void;
};

export function useTeamMemberAcpInput({
  selectedMemberId,
  selectedSessionId,
  onSendInput,
}: UseTeamMemberAcpInputArgs) {
  const isComposingRef = React.useRef(false);
  const inputHistoryDraftRef = React.useRef("");
  const sendingInputRef = React.useRef(false);
  const [input, setInput] = React.useState("");
  const [inputHistory, setInputHistory] = React.useState<string[]>([]);
  const [inputHistoryCursor, setInputHistoryCursor] = React.useState(-1);
  const [sendingInput, setSendingInput] = React.useState(false);

  const canSendInput = Boolean(
    selectedMemberId.trim() && selectedSessionId?.trim() && onSendInput
  );

  const resetInputState = React.useCallback(() => {
    setInput("");
    setInputHistory([]);
    setInputHistoryCursor(-1);
    inputHistoryDraftRef.current = "";
  }, []);

  React.useEffect(() => {
    resetInputState();
  }, [resetInputState, selectedMemberId, selectedSessionId]);

  const sendMemberInput = React.useCallback(async (
    rawText: string,
    options?: {
      recordHistory?: boolean;
      clearComposer?: boolean;
    }
  ) => {
    const text = rawText.trim();
    if (!text || !selectedSessionId || !onSendInput || sendingInputRef.current) {
      return;
    }
    sendingInputRef.current = true;
    setSendingInput(true);
    try {
      await onSendInput(text, selectedSessionId);
      if (options?.recordHistory) {
        setInputHistory((prev) => pushInputHistory(prev, text));
        setInputHistoryCursor(-1);
      }
      if (options?.clearComposer) {
        inputHistoryDraftRef.current = "";
        setInput("");
      }
    } finally {
      sendingInputRef.current = false;
      setSendingInput(false);
    }
  }, [onSendInput, selectedSessionId]);

  const handleSendInput = React.useCallback(async () => {
    await sendMemberInput(input, {
      recordHistory: true,
      clearComposer: true,
    });
  }, [input, sendMemberInput]);

  const handleSubmitRequestUserInput = React.useCallback(async (text: string) => {
    await sendMemberInput(text);
  }, [sendMemberInput]);

  const handleInputChange = React.useCallback(
    (value: string) => {
      setInput(value);
      if (inputHistoryCursor >= 0) {
        setInputHistoryCursor(-1);
      }
      inputHistoryDraftRef.current = value;
    },
    [inputHistoryCursor]
  );

  const handleNavigateHistory = React.useCallback(
    (direction: "up" | "down") => {
      if (inputHistory.length === 0) {
        return;
      }
      if (direction === "up") {
        if (inputHistoryCursor < 0) {
          inputHistoryDraftRef.current = input;
          setInputHistoryCursor(0);
          setInput(inputHistory[0] ?? "");
          return;
        }
        const nextCursor = Math.min(inputHistory.length - 1, inputHistoryCursor + 1);
        setInputHistoryCursor(nextCursor);
        setInput(inputHistory[nextCursor] ?? "");
        return;
      }
      if (inputHistoryCursor < 0) {
        return;
      }
      if (inputHistoryCursor === 0) {
        setInputHistoryCursor(-1);
        setInput(inputHistoryDraftRef.current);
        return;
      }
      const nextCursor = inputHistoryCursor - 1;
      setInputHistoryCursor(nextCursor);
      setInput(inputHistory[nextCursor] ?? "");
    },
    [input, inputHistory, inputHistoryCursor]
  );

  const handleSelectHistoryCommand = React.useCallback(
    (value: string) => {
      const nextCursor = inputHistory.findIndex((item) => item === value);
      setInputHistoryCursor(nextCursor);
      setInput(value);
      inputHistoryDraftRef.current = value;
    },
    [inputHistory]
  );

  return {
    isComposingRef,
    input,
    inputHistory,
    sendingInput,
    canSendInput,
    handleSendInput,
    handleSubmitRequestUserInput,
    handleInputChange,
    handleNavigateHistory,
    handleSelectHistoryCommand,
  };
}
