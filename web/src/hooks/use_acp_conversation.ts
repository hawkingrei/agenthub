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
  deriveConversationFreezeCursor,
  windowConversation,
} from "../conversation";
import { isNearBottom } from "../scroll";
import type { SeqComparable } from "../seq_order";
import { createRafThrottle } from "../raf_throttle";

type EventMeta = {
  oldestId: number | null;
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
  conversationVirtualTopSpacer: number;
  conversationVirtualBottomSpacer: number;
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

type ConversationVirtualSlice = {
  items: ConversationItem[];
  offset: number;
  topSpacer: number;
  bottomSpacer: number;
};

type ConversationViewport = {
  top: number;
  height: number;
};

const VIRTUALIZATION_MIN_ITEMS = 140;
const VIRTUALIZATION_OVERSCAN = 14;

export function buildConversationTailKey(conversationMessages: ConversationItem[]): string {
  if (conversationMessages.length === 0) return "empty";
  const last = conversationMessages[conversationMessages.length - 1];
  const base = `${last.kind}:${last.event_id ?? last.seq ?? "na"}`;
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
}

export function shouldLoadOlderFromMeta(
  activeAgent: string | null,
  activeSessionId: string | null,
  eventMeta: Record<string, EventMeta>
): boolean {
  if (!activeAgent) return false;
  const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
  const meta = eventMeta[key];
  if (!meta || meta.loading || !meta.hasMore || meta.oldestId == null) {
    return false;
  }
  return true;
}

export function shouldUseConversationVirtualization(
  conversationStickToBottom: boolean,
  sourceItemsLength: number
): boolean {
  return !conversationStickToBottom && sourceItemsLength >= VIRTUALIZATION_MIN_ITEMS;
}

export function nextConversationViewport(
  prev: ConversationViewport,
  nextTop: number,
  nextHeight: number
): ConversationViewport {
  if (prev.top === nextTop && prev.height === nextHeight) {
    return prev;
  }
  return {
    top: nextTop,
    height: nextHeight,
  };
}

export function shouldAutoLoadConversationHistory(
  acpTab: "conversation" | "debug",
  activeAgent: string | null,
  canLoadOlder: boolean,
  conversationMessageCount: number,
  minMessages: number = 12
): boolean {
  if (acpTab !== "conversation") return false;
  if (!activeAgent) return false;
  if (!canLoadOlder) return false;
  return conversationMessageCount < minMessages;
}

export function normalizeConversationAvgHeightEstimate(
  previous: number,
  scrollHeight: number,
  messageCount: number
): number {
  const count = Math.max(1, messageCount);
  const estimate = scrollHeight / count;
  if (!Number.isFinite(estimate) || estimate <= 0) return previous;
  const normalized = Math.min(220, Math.max(24, estimate));
  if (Math.abs(previous - normalized) < 1) return previous;
  return normalized;
}

export function restoreConversationScrollTop(
  savedTop: number,
  scrollHeight: number,
  clientHeight: number
): number {
  const maxTop = Math.max(0, scrollHeight - clientHeight);
  return Math.min(savedTop, maxTop);
}

export function deriveConversationJumpState(
  acpTab: "conversation" | "debug",
  conversationStickToBottom: boolean,
  conversationPendingCount: number
): { showConversationJump: boolean; showConversationBadge: boolean } {
  const showConversationJump =
    acpTab === "conversation" && !conversationStickToBottom;
  return {
    showConversationJump,
    showConversationBadge: showConversationJump && conversationPendingCount > 0,
  };
}

export function buildVirtualConversationSlice(
  sourceItems: ConversationItem[],
  sourceOffset: number,
  viewportTop: number,
  viewportHeight: number,
  estimatedItemHeight: number,
  overscan: number = VIRTUALIZATION_OVERSCAN
): ConversationVirtualSlice {
  const total = sourceItems.length;
  if (total === 0) {
    return {
      items: sourceItems,
      offset: sourceOffset,
      topSpacer: 0,
      bottomSpacer: 0,
    };
  }
  const itemHeight = Number.isFinite(estimatedItemHeight) && estimatedItemHeight > 0
    ? estimatedItemHeight
    : 48;
  const safeTop = Math.max(0, viewportTop);
  const safeHeight = Math.max(1, viewportHeight);
  const start = Math.max(0, Math.floor(safeTop / itemHeight) - overscan);
  const visibleCount = Math.max(36, Math.ceil(safeHeight / itemHeight) + overscan * 2);
  const end = Math.min(total, start + visibleCount);
  return {
    items: sourceItems.slice(start, end),
    offset: sourceOffset + start,
    topSpacer: Math.max(0, Math.round(start * itemHeight)),
    bottomSpacer: Math.max(0, Math.round((total - end) * itemHeight)),
  };
}

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
  const conversationScrollThrottleRef = useRef<ReturnType<
    typeof createRafThrottle
  > | null>(null);
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
  const [conversationAvgHeight, setConversationAvgHeight] = useState(48);
  const [conversationViewport, setConversationViewport] = useState({
    top: 0,
    height: 0,
  });
  const didAutoAlignConversationRef = useRef(false);
  const [conversationStickToBottom, setConversationStickToBottom] = useState(true);
  const [conversationFrozen, setConversationFrozen] = useState(false);
  const [conversationFreezeCursor, setConversationFreezeCursor] = useState<
    SeqComparable | null
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
  const conversationTailKey = useMemo(
    () => buildConversationTailKey(conversationMessages),
    [conversationMessages]
  );
  const isFrozenView = conversationFrozen && conversationFrozenItems.length > 0;
  const conversationSourceItems = isFrozenView
    ? conversationFrozenItems
    : conversationWindow.items;
  const conversationSourceOffset = isFrozenView ? 0 : conversationWindow.offset;
  const shouldVirtualizeConversation = shouldUseConversationVirtualization(
    conversationStickToBottom,
    conversationSourceItems.length
  );
  const conversationVirtualSlice = useMemo(() => {
    if (!shouldVirtualizeConversation) {
      return {
        items: conversationSourceItems,
        offset: conversationSourceOffset,
        topSpacer: 0,
        bottomSpacer: 0,
      };
    }
    return buildVirtualConversationSlice(
      conversationSourceItems,
      conversationSourceOffset,
      conversationViewport.top,
      conversationViewport.height,
      conversationAvgHeight
    );
  }, [
    shouldVirtualizeConversation,
    conversationSourceItems,
    conversationSourceOffset,
    conversationViewport.top,
    conversationViewport.height,
    conversationAvgHeight,
  ]);
  const conversationRenderItems = conversationVirtualSlice.items;
  const collapseCutoff = Math.max(0, conversationMessages.length - 50);
  const shouldAutoCollapse = conversationMessages.length > 50;

  const shouldLoadOlder = useCallback(() => {
    return shouldLoadOlderFromMeta(activeAgent, activeSessionId, eventMeta);
  }, [activeAgent, activeSessionId, eventMeta]);

  const syncConversationViewport = useCallback(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    setConversationViewport((prev) => {
      const nextTop = el.scrollTop;
      const nextHeight = el.clientHeight;
      return nextConversationViewport(prev, nextTop, nextHeight);
    });
  }, []);

  const prepareForLoadOlder = () => {
    const el = acpConversationRef.current;
    if (!el) return;
    pendingScrollAdjustRef.current = {
      prevHeight: el.scrollHeight,
      prevTop: el.scrollTop,
    };
  };

  const jumpToConversationBottom = useCallback(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    syncConversationViewport();
    acpStickToBottomRef.current = true;
    didAutoAlignConversationRef.current = true;
    setConversationStickToBottom(true);
    setConversationFrozen(false);
    setConversationFreezeCursor(null);
    setConversationFrozenItems([]);
    setConversationPendingCount(0);
  }, [syncConversationViewport]);

  const handleConversationScrollNow = useCallback(() => {
    const el = acpConversationRef.current;
    if (!el) return;
    syncConversationViewport();
    const stick = isNearBottom(el.scrollHeight, el.scrollTop, el.clientHeight);
    acpStickToBottomRef.current = stick;
    if (stick !== conversationStickToBottom) {
      if (!stick) {
        const maxCursor = deriveConversationFreezeCursor(conversationMessages);
        const frozen = applyConversationFreeze(conversationMessages, maxCursor);
        setConversationFrozen(true);
        setConversationFrozenItems(frozen.frozen);
        setConversationFreezeCursor(maxCursor);
        setConversationPendingCount(frozen.pending);
      } else {
        setConversationFrozen(false);
        setConversationFreezeCursor(null);
        setConversationFrozenItems([]);
        setConversationPendingCount(0);
      }
      setConversationStickToBottom(stick);
    }
    if (el.scrollTop < 80 && shouldLoadOlder()) {
      prepareForLoadOlder();
      onLoadOlder();
    }
  }, [
    conversationStickToBottom,
    conversationMessages,
    onLoadOlder,
    syncConversationViewport,
    shouldLoadOlder,
  ]);

  const handleConversationScroll = useCallback(() => {
    const throttle = conversationScrollThrottleRef.current;
    if (!throttle) {
      handleConversationScrollNow();
      return;
    }
    throttle.schedule();
  }, [handleConversationScrollNow]);

  useEffect(() => {
    if (
      typeof window === "undefined" ||
      typeof window.requestAnimationFrame !== "function"
    ) {
      conversationScrollThrottleRef.current = null;
      return;
    }
    conversationScrollThrottleRef.current = createRafThrottle(
      handleConversationScrollNow,
      {
        requestAnimationFrame: window.requestAnimationFrame.bind(window),
        cancelAnimationFrame:
          typeof window.cancelAnimationFrame === "function"
            ? window.cancelAnimationFrame.bind(window)
            : undefined,
      }
    );
    return () => {
      conversationScrollThrottleRef.current?.cancel();
      conversationScrollThrottleRef.current = null;
    };
  }, [handleConversationScrollNow]);

  useEffect(() => {
    if (
      !shouldAutoLoadConversationHistory(
        acpTab,
        activeAgent,
        shouldLoadOlder(),
        conversationMessages.length
      )
    ) {
      return;
    }
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
    syncConversationViewport();
  }, [
    conversationWindow.items.length,
    acpTab,
    conversationStickToBottom,
    syncConversationViewport,
  ]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationStickToBottom) return;
    const el = acpConversationRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    syncConversationViewport();
  }, [conversationTailKey, acpTab, conversationStickToBottom, syncConversationViewport]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    syncConversationViewport();
  }, [acpTab, conversationTailKey, syncConversationViewport]);

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
    el.scrollTop = restoreConversationScrollTop(
      saved.top,
      el.scrollHeight,
      el.clientHeight
    );
  }, [acpTab, conversationStickToBottom, conversationTailKey, jumpToConversationBottom]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationStickToBottom) return;
    const el = acpConversationRef.current;
    if (!el) return;
    setConversationAvgHeight((prev) =>
      normalizeConversationAvgHeightEstimate(
        prev,
        el.scrollHeight,
        conversationMessages.length
      )
    );
  }, [conversationMessages.length, acpTab, conversationStickToBottom]);

  useEffect(() => {
    const el = acpConversationRef.current;
    const pending = pendingScrollAdjustRef.current;
    if (!el || !pending) return;
    const nextHeight = el.scrollHeight;
    el.scrollTop = nextHeight - pending.prevHeight + pending.prevTop;
    pendingScrollAdjustRef.current = null;
    syncConversationViewport();
  }, [acpView.messages.length, syncConversationViewport]);

  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!conversationFrozen) return;
    const { frozen, pending } = applyConversationFreeze(
      conversationMessages,
      conversationFreezeCursor
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
    conversationFreezeCursor,
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
    jumpToConversationBottom,
  ]);

  const { showConversationJump, showConversationBadge } = deriveConversationJumpState(
    acpTab,
    conversationStickToBottom,
    conversationPendingCount
  );

  return {
    acpConversationRef,
    conversationRenderItems,
    conversationWindowOffset: conversationVirtualSlice.offset,
    conversationVirtualTopSpacer: conversationVirtualSlice.topSpacer,
    conversationVirtualBottomSpacer: conversationVirtualSlice.bottomSpacer,
    isFrozenView,
    collapseCutoff,
    shouldAutoCollapse,
    conversationStickToBottom,
    conversationPendingCount,
    conversationAvgHeight,
    showConversationJump,
    showConversationBadge,
    jumpToConversationBottom,
    handleConversationScroll,
  };
}
