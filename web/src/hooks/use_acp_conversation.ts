import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type RefObject,
} from "react";
import { AcpView } from "../acp";
import {
  applyConversationFreeze,
  buildConversationMessages,
  ConversationItem,
  deriveConversationFreezeMaxSeq,
  windowConversation,
} from "../conversation";
import { isNearBottom } from "../scroll";

type EventMeta = {
  oldestSeq: string | null;
  hasMore: boolean;
  loading: boolean;
  loaded: boolean;
};

type UseAcpConversationArgs = {
  acpView: AcpView;
  activeAgent: string | null;
  activeSessionId: string | null;
  acpTab: "conversation" | "debug";
  eventMeta: Record<string, EventMeta>;
  isAgentActive: boolean;
  onLoadOlder: () => void;
};

type UseAcpConversationResult = {
  acpConversationRef: RefObject<HTMLDivElement>;
  conversationRenderItems: ConversationItem[];
  conversationWindowOffset: number;
  isFrozenView: boolean;
  collapseCutoff: number;
  shouldAutoCollapse: boolean;
  conversationStickToBottom: boolean;
  conversationPendingCount: number;
  conversationAvgHeight: number;
  showConversationJump: boolean;
  showConversationBadge: boolean;
  jumpToConversationBottom: () => void;
  handleConversationScroll: () => void;
};

export function useAcpConversation({
  acpView,
  activeAgent,
  activeSessionId,
  acpTab,
  eventMeta,
  isAgentActive,
  onLoadOlder,
}: UseAcpConversationArgs): UseAcpConversationResult {
  const acpConversationRef = useRef<HTMLDivElement | null>(null);
  const acpStickToBottomRef = useRef(true);
  const conversationScrollRef = useRef<{
    top: number;
    height: number;
    clientHeight: number;
    stickToBottom: boolean;
  } | null>(null);
  const pendingScrollAdjustRef = useRef<{
    prevHeight: number;
    prevTop: number;
  } | null>(null);
  const conversationAvgHeightRef = useRef(48);
  const didAutoAlignConversationRef = useRef(false);
  const [conversationStickToBottom, setConversationStickToBottom] = useState(true);
  const [conversationFrozen, setConversationFrozen] = useState(false);
  const [conversationFreezeMaxSeq, setConversationFreezeMaxSeq] = useState<
    string | null
  >(null);
  const [conversationFrozenItems, setConversationFrozenItems] = useState<
    ConversationItem[]
  >([]);
  const [conversationPendingCount, setConversationPendingCount] = useState(0);
  const conversationMessages = useMemo<ConversationItem[]>(
    () =>
      buildConversationMessages(
        acpView.messages,
        acpView.toolCalls,
        acpView.plan,
        activeSessionId
      ),
    [acpView.messages, acpView.toolCalls, acpView.plan, activeSessionId]
  );
  const conversationWindow = useMemo(
    () => windowConversation(conversationMessages, conversationStickToBottom, 200),
    [conversationMessages, conversationStickToBottom]
  );
  const conversationTailKey = useMemo(() => {
    if (conversationMessages.length === 0) return "empty";
    const last = conversationMessages[conversationMessages.length - 1];
    const base = `${last.kind}:${last.seq ?? "na"}`;
    if (last.kind === "tool_call") {
      const contentLen = last.content?.length ?? 0;
      const terminalLen = last.terminal_output?.length ?? 0;
      const rawInLen = last.raw_input
        ? JSON.stringify(last.raw_input).length
        : 0;
      const rawOutLen = last.raw_output
        ? JSON.stringify(last.raw_output).length
        : 0;
      return `${base}:${contentLen}:${terminalLen}:${rawInLen}:${rawOutLen}`;
    }
    return `${base}:${last.text?.length ?? 0}`;
  }, [conversationMessages]);
  const conversationRenderItems =
    conversationFrozen && conversationFrozenItems.length > 0
      ? conversationFrozenItems
      : conversationWindow.items;
  const isFrozenView = conversationFrozen && conversationFrozenItems.length > 0;
  const collapseCutoff = Math.max(0, conversationMessages.length - 50);
  const shouldAutoCollapse = conversationMessages.length > 50;

  const shouldLoadOlder = useCallback(() => {
    if (!activeAgent) return false;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const meta = eventMeta[key];
    if (!meta || meta.loading || !meta.hasMore || meta.oldestSeq == null) {
      return false;
    }
    return true;
  }, [activeAgent, activeSessionId, eventMeta]);

  const prepareForLoadOlder = () => {
    const el = acpConversationRef.current;
    if (!el) return;
    pendingScrollAdjustRef.current = {
      prevHeight: el.scrollHeight,
      prevTop: el.scrollTop,
    };
  };

  const jumpToConversationBottom = () => {
    const el = acpConversationRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    acpStickToBottomRef.current = true;
    didAutoAlignConversationRef.current = true;
    setConversationStickToBottom(true);
    setConversationFrozen(false);
    setConversationFreezeMaxSeq(null);
    setConversationFrozenItems([]);
    setConversationPendingCount(0);
  };

  const handleConversationScroll = () => {
    const el = acpConversationRef.current;
    if (!el) return;
    const stick = isNearBottom(el.scrollHeight, el.scrollTop, el.clientHeight);
    acpStickToBottomRef.current = stick;
    if (stick !== conversationStickToBottom) {
      if (!stick) {
        const maxSeq = deriveConversationFreezeMaxSeq(conversationMessages);
        const frozen = applyConversationFreeze(conversationMessages, maxSeq);
        setConversationFrozen(true);
        setConversationFrozenItems(frozen.frozen);
        setConversationFreezeMaxSeq(maxSeq);
        setConversationPendingCount(frozen.pending);
      } else {
        setConversationFrozen(false);
        setConversationFreezeMaxSeq(null);
        setConversationFrozenItems([]);
        setConversationPendingCount(0);
      }
      setConversationStickToBottom(stick);
    }
    if (el.scrollTop < 80 && shouldLoadOlder()) {
      prepareForLoadOlder();
      onLoadOlder();
    }
  };

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!activeAgent) return;
    if (!shouldLoadOlder()) return;
    const minMessages = 12;
    if (conversationMessages.length >= minMessages) return;
    prepareForLoadOlder();
    onLoadOlder();
  }, [
    conversationMessages.length,
    acpTab,
    activeAgent,
    activeSessionId,
    eventMeta,
    onLoadOlder,
    shouldLoadOlder,
  ]);

  useEffect(() => {
    didAutoAlignConversationRef.current = false;
  }, [activeAgent, activeSessionId]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    const el = acpConversationRef.current;
    if (!el) return;
    if (!conversationStickToBottom) return;
    el.scrollTop = el.scrollHeight;
  }, [conversationWindow.items.length, acpTab, conversationStickToBottom]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationStickToBottom) return;
    const el = acpConversationRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [conversationTailKey, acpTab, conversationStickToBottom]);

  useEffect(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    if (acpTab !== "conversation") {
      conversationScrollRef.current = {
        top: el.scrollTop,
        height: el.scrollHeight,
        clientHeight: el.clientHeight,
        stickToBottom: conversationStickToBottom,
      };
      return;
    }
    const saved = conversationScrollRef.current;
    if (!saved) return;
    if (saved.stickToBottom) {
      jumpToConversationBottom();
      return;
    }
    acpStickToBottomRef.current = false;
    setConversationStickToBottom(false);
    const maxTop = Math.max(0, el.scrollHeight - el.clientHeight);
    el.scrollTop = Math.min(saved.top, maxTop);
  }, [acpTab, conversationStickToBottom, conversationTailKey]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationStickToBottom) return;
    const el = acpConversationRef.current;
    if (!el) return;
    const count = Math.max(1, conversationMessages.length);
    const estimate = el.scrollHeight / count;
    if (Number.isFinite(estimate) && estimate > 0) {
      conversationAvgHeightRef.current = Math.min(220, Math.max(24, estimate));
    }
  }, [conversationMessages.length, acpTab, conversationStickToBottom]);

  useEffect(() => {
    const el = acpConversationRef.current;
    const pending = pendingScrollAdjustRef.current;
    if (!el || !pending) return;
    const nextHeight = el.scrollHeight;
    el.scrollTop = nextHeight - pending.prevHeight + pending.prevTop;
    pendingScrollAdjustRef.current = null;
  }, [acpView.messages.length]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationFrozen) return;
    const { frozen, pending } = applyConversationFreeze(
      conversationMessages,
      conversationFreezeMaxSeq
    );
    if (frozen.length !== conversationFrozenItems.length) {
      setConversationFrozenItems(frozen);
    }
    if (pending !== conversationPendingCount) {
      setConversationPendingCount(pending);
    }
  }, [
    conversationMessages,
    conversationFrozen,
    conversationFreezeMaxSeq,
    acpTab,
    conversationFrozenItems.length,
    conversationPendingCount,
  ]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!acpView.hasAcp) return;
    if (!activeAgent) return;
    if (isAgentActive) return;
    if (didAutoAlignConversationRef.current) return;
    if (conversationMessages.length === 0) return;
    jumpToConversationBottom();
    didAutoAlignConversationRef.current = true;
  }, [
    acpTab,
    acpView.hasAcp,
    activeAgent,
    isAgentActive,
    conversationMessages.length,
  ]);

  const showConversationJump =
    acpTab === "conversation" && !conversationStickToBottom;
  const showConversationBadge =
    showConversationJump && conversationPendingCount > 0;

  return {
    acpConversationRef,
    conversationRenderItems,
    conversationWindowOffset: conversationWindow.offset,
    isFrozenView,
    collapseCutoff,
    shouldAutoCollapse,
    conversationStickToBottom,
    conversationPendingCount,
    conversationAvgHeight: conversationAvgHeightRef.current,
    showConversationJump,
    showConversationBadge,
    jumpToConversationBottom,
    handleConversationScroll,
  };
}
